use super::*;
use crate::CraftingView;

#[test]
fn creative_catalog_has_four_track_and_switch_woods_without_legacy_curve_items() {
    let mut palette = BlockPalette::default();
    palette.choices.extend((196..=205).map(|id| BlockChoice {
        id,
        name: format!("Rail {id}"),
        identifier: ItemKind::Block(id).identifier(),
        description: String::new(),
    }));
    let mut inventory = terra_core::Inventory::default();
    assert_eq!(inventory.insert(ItemKind::Block(197), 2), 0);
    let creative = available_items(GameMode::Creative, &inventory, &palette);
    for wood in [
        terra_core::WoodSpecies::Oak,
        terra_core::WoodSpecies::Birch,
        terra_core::WoodSpecies::Spruce,
        terra_core::WoodSpecies::Cherry,
    ] {
        for material in [196, 202, 203] {
            let item = ItemKind::for_wood(material, wood);
            assert_eq!(
                creative
                    .iter()
                    .filter(|&&candidate| candidate == item)
                    .count(),
                1
            );
            assert!(catalog_item(item));
            assert!(!view::item_view(item, 1, &palette).empty);
        }
    }
    for material in (197..=201).chain(204..=205) {
        assert!(!creative.contains(&ItemKind::Block(material)));
        assert!(!catalog_item(ItemKind::Block(material)));
    }
    assert_eq!(
        available_items(GameMode::Survival, &inventory, &palette),
        vec![ItemKind::Block(197)]
    );
}

#[test]
fn reference_recipe_book_and_catalog_clicks_are_read_only_and_filter_closes_after_selection() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    app.world_mut()
        .write_message(crate::CraftingAction::OpenBook);
    app.update();
    assert!(app.world().resource::<CraftingView>().recipe_open);
    assert!(
        !app.world()
            .resource::<CraftingView>()
            .recipe_grid
            .is_empty()
    );
    action(
        &mut app,
        InventoryAction::Pointer {
            command: "catalog".into(),
            index: None,
            item: "block-16".into(),
            secondary: false,
        },
    );
    assert!(
        requests(&mut app).is_empty(),
        "a recipe catalog click must not grant a Creative stack"
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert_eq!(
        app.world().resource::<CraftingView>().recipe_output[0].item,
        Some(ItemKind::Block(16))
    );
    action(&mut app, InventoryAction::ToggleCategories);
    assert!(app.world().resource::<InventoryView>().category_dropdown);
    app.update();
    assert!(app.world().resource::<InventoryView>().category_dropdown);
    action(&mut app, InventoryAction::Category("wood".into()));
    assert!(!app.world().resource::<InventoryView>().category_dropdown);
    assert!(
        app.world()
            .resource::<InventoryView>()
            .browser_items
            .iter()
            .all(|item| item.category == "wood")
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
}

#[path = "inventory_alchemy_tests.rs"]
mod alchemy_tests;

#[test]
fn tmi_offers_only_white_textile_bases_but_keeps_the_pot_and_existing_bed_contract() {
    for base in [
        terra_core::textiles::WHITE_WOOL,
        terra_core::textiles::WHITE_CARPET,
    ] {
        assert!(catalog_item(ItemKind::Block(base)));
        for color in terra_core::beds::DyeColor::ALL {
            let id = terra_core::textiles::dyed(base, color).unwrap();
            assert_eq!(
                catalog_item(ItemKind::Block(id)),
                color == terra_core::beds::DyeColor::White
            );
        }
    }
    assert!(catalog_item(ItemKind::Block(
        terra_core::flower_pot::MATERIAL
    )));
    assert!(catalog_item(ItemKind::Block(71)));
}

#[test]
fn farming_browser_shows_real_seed_and_food_items_without_internal_growth_props() {
    let palette = BlockPalette::default();
    let items = available_items(
        GameMode::Creative,
        &terra_core::Inventory::default(),
        &palette,
    );
    for crop in terra_core::CropItem::ALL {
        let item = ItemKind::Crop(crop);
        assert_eq!(
            items.iter().filter(|&&candidate| candidate == item).count(),
            1
        );
        assert!(preview_key(item).starts_with("Item preview teramic::"));
        let view = view::item_view(item, 1, &palette);
        assert!(!view.empty && !view.name.is_empty() && !view.preview_key.is_empty());
    }
    assert!(!items.iter().any(|item| matches!(item, ItemKind::Prop(kind) if terra_core::CropKind::from_prop(*kind).is_some())));
}
use terra_core::{ControlsConfig, InputBinding};
use terra_network::{
    client::ContainerRequest,
    protocol::inventory::{ContainerCommand, ContainerHalf, ContainerSnapshot, InventoryCommand},
};
use terra_player::interaction::BlockChoice;

fn filtered_funnel_snapshot(anchor: [i64; 3]) -> ContainerSnapshot {
    ContainerSnapshot {
        anchor,
        workbench: None,
        view: ContainerView::Primary,
        attached_campfire: false,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        furnace: None,
        crusher: None,
        mixer: None,
        filter: Some(terra_core::logistics::FilterConfig::default()),
        distributor: None,
        halves: Vec::new(),
    }
}

#[test]
fn filtered_funnel_selects_exact_rgb_ghosts_without_moving_or_creating_items() {
    for mode in [GameMode::Survival, GameMode::Creative] {
        for material in [157, 159] {
            let mut app = app(mode);
            let anchor = [-12, 18, 2];
            let (section, local) = terra_voxel::global_to_section(anchor);
            let mut data = terra_voxel::BlockSection::empty(section).unwrap();
            data.set_block(local, material).unwrap();
            let mut chunks = terra_world::streaming::ChunkStore::default();
            chunks.loaded.insert(
                section,
                terra_world::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            app.insert_resource(chunks);
            let rgb = ItemKind::TintedBlock {
                material: 103,
                wood: terra_core::WoodSpecies::Oak,
                color: [17, 120, 211],
            };
            {
                let mut session = app.world_mut().resource_mut::<ClientSession>();
                let mut snapshot = filtered_funnel_snapshot(anchor);
                snapshot.tunnel_random = (material == 159).then_some(false);
                session.container = Some(snapshot);
                session
                    .inventory
                    .slots
                    .resize(terra_core::INVENTORY_SLOTS, terra_core::ItemStack::EMPTY);
                session.inventory.slots[9] = terra_core::ItemStack {
                    item: rgb,
                    count: 7,
                };
            }
            action(&mut app, InventoryAction::Toggle);
            let original = app.world().resource::<ClientSession>().inventory.clone();
            let view = app.world().resource::<InventoryView>();
            assert!(view.filtered_funnel && !view.container && view.filter_allowed);
            assert!(view.container_items.is_empty());
            assert_eq!(
                view.title,
                match material {
                    159 => "Tunnel (Filtered)",
                    _ => "Funnel (Filtered)",
                }
            );
            assert_eq!(view.funnel_filter.len(), 1);
            assert!(view.funnel_filter[0].empty);
            action(
                &mut app,
                InventoryAction::Pointer {
                    command: "filter".into(),
                    index: Some(0),
                    item: String::new(),
                    secondary: false,
                },
            );
            assert!(app.world().resource::<InventoryView>().filter_selecting);
            action(
                &mut app,
                InventoryAction::Pointer {
                    command: "slot".into(),
                    index: Some(9),
                    item: item_id(rgb),
                    secondary: false,
                },
            );
            let sent = requests(&mut app);
            assert_eq!(sent.len(), 1);
            assert_eq!(
                sent[0].action,
                ServerAction::SetFunnelFilter {
                    station: anchor,
                    filter: terra_core::logistics::FilterConfig {
                        item: Some(rgb),
                        allowed: true
                    }
                }
            );
            let view = app.world().resource::<InventoryView>();
            assert!(!view.filter_selecting && view.filter_has_item);
            assert_eq!(
                view.title,
                if material == 159 {
                    "Tunnel (Filtered)"
                } else {
                    "Funnel (Filtered)"
                }
            );
            assert_eq!(view.funnel_filter[0].ghost_preview, preview_key(rgb));
            assert_eq!(view.funnel_filter[0].identifier, rgb.identifier());
            assert!(view.funnel_filter[0].ghost_count.is_empty());
            assert_eq!(app.world().resource::<ClientSession>().inventory, original);
            assert!(!app.world().resource::<crafting::CraftingView>().recipe_open);
        }
    }
}

#[test]
fn logistics_and_alchemy_clicks_open_the_existing_authorized_container_path() {
    for material in [152, 157, 159, 161, 162] {
        let mut app = app(GameMode::Survival);
        let position = [2, 3, 4];
        let (section, local) = terra_voxel::global_to_section(position);
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        data.set_block(local, material).unwrap();
        let mut chunks = terra_world::streaming::ChunkStore::default();
        chunks.loaded.insert(
            section,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(terra_voxel::blocks::BlockHit {
                    position,
                    normal: [0, 0, 1],
                    distance: 2.0,
                }),
            )));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert!(
            requests(&mut app).is_empty(),
            "an unbound mouse button must not open material {material}"
        );
        assert!(!app.world().resource::<InventoryState>().open);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Right);
        app.world_mut()
            .resource_mut::<ControlsConfig>()
            .rebind(GameAction::Place, InputBinding::Mouse(MouseButton::Right))
            .unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert!(
            matches!(requests(&mut app).as_slice(), [InventoryCommand {
            action: ServerAction::OpenContainer { position: actual }, ..
        }] if *actual == position),
            "material {material} uses the same authorized container opening"
        );
    }
}

#[test]
fn filtered_funnel_catalog_and_cursor_configuration_coalesces_and_is_owner_scoped() {
    let mut app = app(GameMode::Survival);
    let anchor = [0, 18, -1];
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(filtered_funnel_snapshot(anchor));
    action(&mut app, InventoryAction::Toggle);
    let original = app.world().resource::<ClientSession>().inventory.clone();
    action(
        &mut app,
        InventoryAction::Pointer {
            command: "catalog".into(),
            index: Some(0),
            item: item_id(ItemKind::Block(3)),
            secondary: false,
        },
    );
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetFunnelFilter {
            station: anchor,
            filter: terra_core::logistics::FilterConfig {
                item: Some(ItemKind::Block(3)),
                allowed: true
            }
        }
    );
    assert!(!app.world().resource::<crafting::CraftingView>().recipe_open);
    action(&mut app, InventoryAction::FilterAllowed(false));
    action(&mut app, InventoryAction::FilterClear);
    assert!(requests(&mut app).is_empty());
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.filter_mode, "Blocked");
    assert!(!view.filter_has_item);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session
            .container
            .as_mut()
            .unwrap()
            .filter
            .as_mut()
            .unwrap()
            .item = Some(ItemKind::Block(3));
    }
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetFunnelFilter {
            station: anchor,
            filter: terra_core::logistics::FilterConfig {
                item: None,
                allowed: false
            }
        }
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, original);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([8, 18, -1], false));
    action(&mut app, InventoryAction::FilterAllowed(true));
    action(&mut app, InventoryAction::FilterClear);
    assert!(requests(&mut app).is_empty());
}

#[test]
fn distributor_filters_keep_three_independent_rgb_ghosts_without_inventory_mutation() {
    use terra_core::logistics::{DistributorConfig, FilterConfig};
    let mut app = app(GameMode::Survival);
    let anchor = [-16, 18, -1];
    let mut snapshot = filtered_funnel_snapshot(anchor);
    snapshot.filter = None;
    snapshot.distributor = Some(DistributorConfig::default());
    let items = [
        ItemKind::Block(3),
        ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184])),
        ItemKind::TintedBlock {
            material: 103,
            wood: terra_core::WoodSpecies::Oak,
            color: [91, 122, 211],
        },
    ];
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.container = Some(snapshot);
        session
            .inventory
            .slots
            .resize(terra_core::INVENTORY_SLOTS, terra_core::ItemStack::EMPTY);
        for (index, item) in items.into_iter().enumerate() {
            session.inventory.slots[index + 9] = terra_core::ItemStack { item, count: 7 };
        }
    }
    action(&mut app, InventoryAction::Toggle);
    let original = app.world().resource::<ClientSession>().inventory.clone();
    assert_eq!(
        app.world().resource::<InventoryView>().title,
        "Belt Distributor"
    );
    assert!(!app.world().resource::<InventoryView>().container);
    assert_eq!(
        app.world()
            .resource::<InventoryView>()
            .distributor_filters
            .len(),
        3
    );
    for (output, item) in items.into_iter().enumerate() {
        let output = output as u8;
        action(
            &mut app,
            InventoryAction::Pointer {
                command: "distributor-filter".into(),
                index: Some(output),
                item: String::new(),
                secondary: false,
            },
        );
        assert!(
            app.world().resource::<InventoryView>().distributor_filters[usize::from(output)]
                .selecting
        );
        action(
            &mut app,
            InventoryAction::Pointer {
                command: "slot".into(),
                index: Some(output + 9),
                item: item_id(item),
                secondary: false,
            },
        );
        let sent = requests(&mut app);
        let filter = FilterConfig {
            item: Some(item),
            allowed: true,
        };
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0].action,
            ServerAction::SetDistributorFilter {
                station: anchor,
                output,
                filter
            }
        );
        let view = app.world().resource::<InventoryView>();
        assert_eq!(
            view.distributor_filters[usize::from(output)].slots[0].ghost_preview,
            preview_key(item)
        );
        assert!(view.distributor_filters[usize::from(output)].slots[0].empty);
        assert!(
            view.distributor_filters[usize::from(output)].slots[0]
                .ghost_count
                .is_empty()
        );
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session
                .container
                .as_mut()
                .unwrap()
                .distributor
                .as_mut()
                .unwrap()
                .filters[usize::from(output)] = filter;
            session.inventory_response += 1;
        }
        app.update();
    }
    action(&mut app, InventoryAction::DistributorAllowed(1, false));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetDistributorFilter {
            station: anchor,
            output: 1,
            filter: FilterConfig {
                item: Some(items[1]),
                allowed: false
            }
        }
    );
    action(&mut app, InventoryAction::DistributorClear(1));
    action(&mut app, InventoryAction::DistributorAllowed(2, false));
    let view = app.world().resource::<InventoryView>();
    assert!(!view.distributor_filters[1].has_item);
    assert!(!view.distributor_filters[1].allowed);
    assert_eq!(
        view.distributor_filters[0].slots[0].identifier,
        items[0].identifier()
    );
    assert_eq!(
        view.distributor_filters[2].slots[0].identifier,
        items[2].identifier()
    );
    assert!(!view.distributor_filters[2].allowed);
    assert_eq!(app.world().resource::<ClientSession>().inventory, original);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([8, 18, -1], false));
    action(&mut app, InventoryAction::DistributorAllowed(0, false));
    action(&mut app, InventoryAction::DistributorClear(0));
    assert!(requests(&mut app).is_empty());
}

#[test]
fn exact_rgb_fabrics_and_beds_keep_hex_tooltips_and_stay_out_of_tmi() {
    use terra_core::beds::DyeColor;
    let palette = BlockPalette::default();
    for material in [103, 106, 148, 71, 72, 73] {
        let source = ItemKind::Block(material);
        let rgb = terra_core::tints::recolored(source, DyeColor::Rgb([12, 34, 56])).unwrap();
        let second = terra_core::tints::recolored(source, DyeColor::Rgb([12, 34, 57])).unwrap();
        assert_ne!(item_id(rgb), item_id(second));
        assert_ne!(preview_key(rgb), preview_key(second));
        let view = view::item_view(rgb, 1, &palette);
        assert!(view.name.contains("#0C2238"));
        assert!(view.identifier.ends_with("#0C2238"));
        assert!(!catalog_item(rgb));
    }
}

#[test]
fn exact_rgb_fabric_recipe_is_generated_on_demand_without_a_catalog_variant() {
    let mut app = app(GameMode::Survival);
    let rgb = [12, 34, 56];
    let dye = ItemKind::Dye(terra_core::beds::DyeColor::Rgb(rgb));
    let item =
        terra_core::tints::recolored(ItemKind::Block(103), terra_core::beds::DyeColor::Rgb(rgb))
            .unwrap();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.insert(item, 1);
        session.inventory.insert(dye, 1);
        session.inventory.insert(ItemKind::Block(103), 1);
    }
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Recipe(item_id(item)));
    let view = app.world().resource::<CraftingView>();
    assert!(view.recipe_open && view.selected);
    assert_eq!(view.recipe_heading, "Coloring Recipe");
    assert_eq!(view.recipe_grid.len(), 2);
    assert_eq!(
        view.recipe_grid[0].preview_key,
        preview_key(ItemKind::Block(103))
    );
    assert_eq!(view.recipe_grid[1].preview_key, preview_key(dye));
    assert_eq!(view.recipe_output[0].preview_key, preview_key(item));
    assert!(!view.can_apply_recipe);
    assert!(!catalog_item(item));
    assert!(requests(&mut app).is_empty());
}

#[test]
fn creative_pick_preserves_exact_rgb_metadata_but_only_grants_one_item() {
    let mut app = app(GameMode::Creative);
    let item = terra_core::tints::recolored(
        ItemKind::Block(103),
        terra_core::beds::DyeColor::Rgb([71, 93, 184]),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .choices
        .push(BlockChoice {
            id: 103,
            name: "Wool".into(),
            identifier: ItemKind::Block(103).identifier(),
            description: String::new(),
        });
    action(&mut app, InventoryAction::PickItem(item));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeSet {
            slot: 0,
            item,
            count: 1
        }
    );
}

#[test]
fn equipment_clicks_and_shift_removal_use_real_inventory_commands() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        InventoryAction::Pointer {
            command: "equipment".into(),
            index: Some(8),
            item: String::new(),
            secondary: true,
        },
    );
    assert!(matches!(
        requests(&mut app).as_slice(),
        [InventoryCommand {
            action: ServerAction::EquipmentClick {
                slot: 8,
                secondary: true
            },
            ..
        }]
    ));
    // Complete the previous request before dispatching another serial transaction.
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    action(
        &mut app,
        InventoryAction::Pointer {
            command: "equipment".into(),
            index: Some(2),
            item: String::new(),
            secondary: false,
        },
    );
    assert!(matches!(
        requests(&mut app).as_slice(),
        [InventoryCommand {
            action: ServerAction::EquipmentQuickMove { slot: 2 },
            ..
        }]
    ));
}

#[test]
fn equipment_view_refreshes_when_stats_arrive_before_the_bag_revision() {
    use terra_core::{ArmorItem, EquipmentSlot};
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    let before = app.world().resource::<ClientSession>().inventory.revision;
    let item = ItemKind::Armor(ArmorItem::ALL[0]);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .stats
        .equipment
        .set(EquipmentSlot::Helmet, Some(item));
    app.update();
    assert_eq!(
        app.world().resource::<ClientSession>().inventory.revision,
        before
    );
    let slot = &app.world().resource::<InventoryView>().equipment[0];
    assert!(!slot.item.empty);
    assert_eq!(slot.item.preview_key, preview_key(item));
    assert!(slot.item.description.contains("Armor:"));
    let serialized = toml::Value::try_from(app.world().resource::<InventoryView>()).unwrap();
    assert_eq!(
        serialized["equipment"][0]["preview_key"].as_str(),
        Some(preview_key(item).as_str())
    );
    assert!(serialized["equipment"][0].get("item").is_none());
}

fn food_input_app(binding: InputBinding) -> App {
    let mut app = app(GameMode::Survival);
    app.add_message::<terra_network::client::ConsumptionRequest>()
        .add_systems(Update, held_consumption.after(actions));
    app.world_mut()
        .resource_mut::<ControlsConfig>()
        .rebind(GameAction::Place, binding)
        .unwrap();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Prop(ResourcePropKind::Mushroom), 3);
    action(&mut app, InventoryAction::Hotbar(0));
    app
}

fn food_requests(app: &mut App) -> Vec<Option<u8>> {
    app.world_mut()
        .resource_mut::<Messages<terra_network::client::ConsumptionRequest>>()
        .drain()
        .map(|request| request.0)
        .collect()
}

fn press_compact_hearth(
    binding: InputBinding,
    material: u16,
    held: ItemKind,
    attached: bool,
) -> App {
    let mut app = food_input_app(binding);
    let position = [3, 1, 1];
    let owner = terra_voxel::SectionPos::default();
    let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
    data.set_block([3, 1, 1], material).unwrap();
    let mut chunks = terra_world::streaming::ChunkStore::default();
    chunks.loaded.insert(
        owner,
        terra_world::streaming::LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut journal = terra_world::edits::EditJournal::default();
    journal.record(position, material).unwrap();
    journal
        .set_object_state(
            position,
            Some(terra_world::edits::BlockObjectState {
                facing: if attached {
                    terra_core::brewing::ATTACHED_FIRE
                } else {
                    0
                },
                ..default()
            }),
        )
        .unwrap();
    assert_eq!(journal.has_attached_campfire(position), attached);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(
            0,
            terra_core::ItemStack {
                item: held,
                count: 1,
            },
        );
    app.insert_resource(ActiveItem(Some(held)))
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(terra_player::TargetWorld(Some(
            terra_player::WorldTarget::Block(terra_voxel::blocks::BlockHit {
                position,
                normal: [0, 1, 0],
                distance: 2.0,
            }),
        )))
        .add_systems(PreUpdate, block_opening_frame);
    app.world_mut()
        .resource_mut::<Messages<InventoryAction>>()
        .clear();
    match binding {
        InputBinding::Key(key) => app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key),
        InputBinding::Mouse(button) => app
            .world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(button),
        InputBinding::Chord { .. } => unreachable!(),
    }
    app.update();
    app
}

#[test]
fn rebound_place_assembles_both_compact_hearth_orders_without_opening_or_blocking_input() {
    for binding in [
        InputBinding::Key(KeyCode::KeyU),
        InputBinding::Mouse(MouseButton::Right),
    ] {
        for (material, held) in [(161, 162), (162, 161)] {
            let mut app = press_compact_hearth(binding, material, ItemKind::Block(held), false);
            assert!(
                !app.world().resource::<PlayerInputBlock>().0,
                "target={material}, held={held}"
            );
            assert!(!app.world().resource::<InventoryState>().open);
            assert!(
                app.world_mut()
                    .resource_mut::<Messages<InventoryAction>>()
                    .drain()
                    .all(|action| !matches!(action, InventoryAction::OpenChest(_)))
            );
            assert!(requests(&mut app).is_empty());
            assert!(
                app.world()
                    .resource::<Messages<ContainerRequest>>()
                    .is_empty()
            );
            assert!(food_requests(&mut app).is_empty());
        }
    }
}

#[test]
fn an_attached_compact_hearth_opens_normally_while_holding_another_campfire() {
    for binding in [
        InputBinding::Key(KeyCode::KeyU),
        InputBinding::Mouse(MouseButton::Right),
    ] {
        let mut app = press_compact_hearth(binding, 161, ItemKind::Block(162), true);
        assert!(app.world().resource::<PlayerInputBlock>().0);
        assert!(app.world().resource::<InventoryState>().open);
        assert!(matches!(
            requests(&mut app).as_slice(),
            [InventoryCommand {
                action: ServerAction::OpenContainer {
                    position: [3, 1, 1]
                },
                ..
            }]
        ));
        assert!(food_requests(&mut app).is_empty());
    }
}

#[test]
fn held_food_preserves_normal_cauldron_and_campfire_opening_on_rebound_place() {
    for binding in [
        InputBinding::Key(KeyCode::KeyU),
        InputBinding::Mouse(MouseButton::Right),
    ] {
        for material in [161, 162] {
            let mut app = press_compact_hearth(
                binding,
                material,
                ItemKind::Prop(ResourcePropKind::Mushroom),
                false,
            );
            assert!(app.world().resource::<PlayerInputBlock>().0);
            assert!(app.world().resource::<InventoryState>().open);
            assert!(matches!(
                requests(&mut app).as_slice(),
                [InventoryCommand {
                    action: ServerAction::OpenContainer {
                        position: [3, 1, 1]
                    },
                    ..
                }]
            ));
            assert!(food_requests(&mut app).is_empty());
        }
    }
}

#[test]
fn using_a_real_nest_takes_priority_over_eating_a_held_food_item() {
    use terra_world::props::{PropHit, PropId, PropKind, WorldProp};
    let mut app = food_input_app(InputBinding::Key(KeyCode::KeyU));
    app.insert_resource(terra_player::TargetWorld(Some(
        terra_player::WorldTarget::Prop(PropHit {
            prop: WorldProp {
                id: PropId(121),
                kind: PropKind::BeeNest,
                position: [3.5, 3.0, 3.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: terra_voxel::SectionPos::default(),
                removed: false,
            },
            distance: 2.0,
        }),
    )));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyU);
    for _ in 0..3 {
        app.update();
        assert!(food_requests(&mut app).is_empty());
    }
    app.world_mut()
        .resource_mut::<terra_player::TargetWorld>()
        .0 = None;
    app.update();
    assert_eq!(food_requests(&mut app), vec![Some(0)]);
}

#[test]
fn a_reachable_crusher_tray_item_takes_priority_but_hidden_or_off_ray_items_do_not() {
    for (drop_x, occluded, collect) in [(2.5, false, true), (4.5, false, false), (2.5, true, false)]
    {
        let mut app = food_input_app(InputBinding::Mouse(MouseButton::Right));
        app.add_systems(PreUpdate, block_opening_frame);
        let position = [2, 2, 2];
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record(position, 99).unwrap();
        journal
            .set_object_state(position, Some(Default::default()))
            .unwrap();
        let mut game = terra_core::GameplayState::default();
        assert!(game.drop_stack(
            [drop_x, 3.13, 2.5],
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 7
            },
            [0.0; 3],
            0.0
        ));
        app.world_mut().resource_mut::<ClientSession>().drops = game.drops;
        let mut chunks = terra_world::streaming::ChunkStore::default();
        let owner = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        data.set_block([2, 2, 2], 99).unwrap();
        data.set_block([4, 2, 2], 99).unwrap();
        journal.record([4, 2, 2], 99).unwrap();
        journal
            .set_object_state([4, 2, 2], Some(Default::default()))
            .unwrap();
        let chest = if occluded { [2, 3, 4] } else { [2, 3, 1] };
        data.set_block(chest.map(|v| v as usize), 29).unwrap();
        journal.record(chest, 29).unwrap();
        journal
            .set_object_state(chest, Some(Default::default()))
            .unwrap();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let eye = Vec3::new(2.5, 3.13, 5.5);
        let hit = terra_world::interaction_shapes::raycast(
            &chunks,
            &journal,
            &Default::default(),
            eye.to_array().map(f64::from),
            [0.0, 0.0, -1.0],
            6.0,
        )
        .unwrap();
        assert_eq!(hit.position, chest);
        app.insert_resource(chunks)
            .insert_resource(journal)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(hit),
            )));
        app.world_mut().spawn((
            terra_player::TerraCamera,
            terra_player::CameraLook::default(),
            Transform::from_translation(eye),
        ));
        {
            let mut context =
                bevy::ecs::system::SystemState::<InventoryContext>::new(app.world_mut());
            let context = context
                .get(app.world())
                .expect("fixture inventory context must be available");
            assert_eq!(carried_item_priority(&context), collect);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(app.world().resource::<PlayerInputBlock>().0, !collect);
        let commands = requests(&mut app);
        if collect {
            assert!(
                commands.is_empty(),
                "clicking the visible tray item must not open a station"
            );
        } else {
            assert!(
                matches!(commands.as_slice(),[InventoryCommand { action:ServerAction::OpenContainer {position},.. }] if *position==chest),
                "a hidden or off-ray tray item must preserve the exact normal station request: {commands:?}"
            );
        }
        assert!(
            app.world()
                .resource::<Messages<ContainerRequest>>()
                .is_empty()
        );
    }
}

#[test]
fn wrench_and_mechanical_parts_take_priority_over_station_and_crank_input() {
    let wrench = ItemKind::Tool(terra_core::ToolKind::Wrench);
    let cases = [29, 65, 71, 99, 100, 166, 167]
        .map(|material| (material, wrench))
        .into_iter()
        .chain([99, 100].into_iter().flat_map(|material| {
            [100, 142, 145, 148].map(|held| (material, ItemKind::Block(held)))
        }))
        .chain([166, 167].into_iter().flat_map(|material| {
            [163, 164, 165, 166, 167, 168, 169].map(|held| (material, ItemKind::Block(held)))
        }))
        .chain([
            (167, ItemKind::GlassBottle),
            (167, ItemKind::Tool(terra_core::ToolKind::Bucket)),
            (167, ItemKind::Tool(terra_core::ToolKind::IronWateringCan)),
            (
                167,
                ItemKind::FilledWateringCan {
                    tool: terra_core::ToolKind::IronWateringCan,
                    units: 2,
                },
            ),
        ]);
    for (material, item) in cases {
        let mut app = food_input_app(InputBinding::Mouse(MouseButton::Right));
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .creative_set(0, terra_core::ItemStack { item, count: 1 });
        app.insert_resource(ActiveItem(Some(item)))
            .init_resource::<terra_player::CameraInputState>()
            .init_resource::<terra_network::client::CrankInput>()
            .add_systems(
                Update,
                drive_crusher.after(actions).before(held_consumption),
            )
            .add_systems(PreUpdate, block_opening_frame);
        app.world_mut()
            .resource_mut::<terra_player::CameraInputState>()
            .captured = true;
        let position = [2, 2, 2];
        let mut chunks = terra_world::streaming::ChunkStore::default();
        let owner = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        data.set_block([2, 2, 2], material).unwrap();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(terra_voxel::blocks::BlockHit {
                    position,
                    normal: [0, 0, 1],
                    distance: 2.0,
                }),
            )));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert!(
            !app.world().resource::<InventoryState>().open,
            "material {material}"
        );
        assert!(!app.world().resource::<PlayerInputBlock>().0);
        assert!(
            app.world()
                .resource::<terra_player::CameraInputState>()
                .captured
        );
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            None
        );
        assert!(
            app.world()
                .resource::<Messages<terra_network::client::ObjectInteractionRequest>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<ContainerRequest>>()
                .is_empty()
        );
    }
}

fn rail_priority_app(material: u16) -> App {
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::{
        edits::{BlockObjectState, EditJournal},
        streaming::{ChunkStore, LoadedSection},
    };
    let mut app = food_input_app(InputBinding::Mouse(MouseButton::Right));
    let item = ItemKind::for_wood(196, terra_core::WoodSpecies::Birch);
    app.world_mut().resource_mut::<ClientSession>().allow_build = true;
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(0, terra_core::ItemStack { item, count: 4 });
    app.insert_resource(ActiveItem(Some(item)))
        .init_resource::<terra_player::CameraInputState>()
        .init_resource::<terra_network::client::CrankInput>()
        .add_systems(PreUpdate, block_opening_frame)
        .add_systems(
            Update,
            drive_crusher.after(actions).before(held_consumption),
        );
    app.world_mut()
        .resource_mut::<terra_player::CameraInputState>()
        .captured = true;
    let position = [3, 1, 1];
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    data.set_block([3, 1, 1], material).unwrap();
    let mut journal = EditJournal::default();
    if material == terra_core::crusher::CRANK_MATERIAL {
        for (cell, block, peer, secondary) in [
            ([1, 1, 1], 99, [2, 1, 1], false),
            ([2, 1, 1], 99, [1, 1, 1], true),
            (position, material, [1, 1, 1], false),
        ] {
            data.set_block(cell.map(|value| value as usize), block)
                .unwrap();
            journal.record(cell, block).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..default()
                    }),
                )
                .unwrap();
        }
        assert!(journal.crank_anchor(position).is_some());
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(terra_player::TargetWorld(Some(
            terra_player::WorldTarget::Block(BlockHit {
                position,
                normal: [0, 1, 0],
                distance: 2.0,
            }),
        )));
    app
}

#[test]
fn held_track_use_owns_mode_input_before_station_faucet_and_crank_even_while_held() {
    for material in [28, 29, 65, 167, terra_core::crusher::CRANK_MATERIAL] {
        for use_key in [KeyCode::KeyR, KeyCode::KeyY] {
            let mut app = rail_priority_app(material);
            app.world_mut()
                .resource_mut::<ControlsConfig>()
                .rebind(GameAction::Use, InputBinding::Key(use_key))
                .unwrap();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(use_key);
            for frame in 0..2 {
                if frame == 1 {
                    app.world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .clear_just_pressed(use_key);
                }
                app.update();
                assert!(
                    !app.world().resource::<InventoryState>().open,
                    "Use must not open station {material}"
                );
                assert!(
                    !app.world().resource::<PlayerInputBlock>().0,
                    "UI must leave modecycle input available"
                );
                assert!(
                    app.world()
                        .resource::<Messages<ContainerRequest>>()
                        .is_empty()
                );
                assert!(
                    app.world()
                        .resource::<Messages<terra_network::client::ObjectInteractionRequest>>()
                        .is_empty()
                );
                assert_eq!(
                    app.world()
                        .resource::<terra_network::client::CrankInput>()
                        .0,
                    None
                );
                assert!(food_requests(&mut app).is_empty());
            }
        }
    }
}

#[test]
fn held_track_place_keeps_existing_station_faucet_and_crank_actions() {
    for material in [28, 29, 65, 167, terra_core::crusher::CRANK_MATERIAL] {
        let mut app = rail_priority_app(material);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(
            app.world().resource::<InventoryState>().open,
            matches!(material, 28 | 29)
        );
        let objects = app
            .world_mut()
            .resource_mut::<Messages<terra_network::client::ObjectInteractionRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(objects.len(), usize::from(matches!(material, 65 | 167)));
        if let Some(object) = objects.first() {
            assert_eq!(object.expected, material);
        }
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            (material == terra_core::crusher::CRANK_MATERIAL).then_some([3, 1, 1])
        );
    }
}

#[test]
fn mounted_crank_uses_rebound_hold_without_eating_or_unlocking_the_camera() {
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::{
        edits::{BlockObjectState, EditJournal},
        streaming::{ChunkStore, LoadedSection},
    };
    for binding in [
        InputBinding::Key(KeyCode::KeyU),
        InputBinding::Mouse(MouseButton::Right),
    ] {
        let mut app = food_input_app(binding);
        app.init_resource::<terra_network::client::CrankInput>()
            .init_resource::<terra_player::CameraInputState>()
            .add_systems(
                Update,
                drive_crusher.after(actions).before(held_consumption),
            );
        app.world_mut()
            .resource_mut::<terra_player::CameraInputState>()
            .captured = true;
        let mut journal = EditJournal::default();
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        for (position, material, peer, secondary) in [
            ([1, 1, 1], 99, [2, 1, 1], false),
            ([2, 1, 1], 99, [1, 1, 1], true),
            ([3, 1, 1], 100, [1, 1, 1], false),
        ] {
            journal.record(position, material).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..default()
                    }),
                )
                .unwrap();
            data.set_block(position.map(|p| p as usize), material)
                .unwrap();
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(journal)
            .insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(BlockHit {
                    position: [3, 1, 1],
                    normal: [1, 0, 0],
                    distance: 2.0,
                }),
            )));
        match binding {
            InputBinding::Key(key) => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key),
            InputBinding::Mouse(button) => app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button),
            InputBinding::Chord { .. } => unreachable!(),
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            Some([3, 1, 1])
        );
        assert!(food_requests(&mut app).is_empty());
        assert!(
            app.world()
                .resource::<terra_player::CameraInputState>()
                .captured
        );
        assert!(!app.world().resource::<PlayerInputBlock>().0);
        assert!(!app.world().resource::<InventoryState>().open);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            None
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            Some([3, 1, 1])
        );
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            None
        );
        app.world_mut().resource_mut::<crate::SettingsView>().open = false;
        app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Spectator;
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            None
        );
        app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Survival;
        app.world_mut()
            .resource_mut::<terra_player::TargetWorld>()
            .0 = None;
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::CrankInput>()
                .0,
            None
        );
    }
}

#[test]
fn rebound_place_key_and_mouse_send_held_food_pulses_and_cancel_on_release_or_menu() {
    for binding in [
        InputBinding::Key(KeyCode::KeyU),
        InputBinding::Mouse(MouseButton::Right),
    ] {
        let mut app = food_input_app(binding);
        let press = |app: &mut App, held| match binding {
            InputBinding::Key(key) => {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                if held {
                    keys.press(key);
                } else {
                    keys.release(key);
                }
            }
            InputBinding::Mouse(button) => {
                let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
                if held {
                    mouse.press(button);
                } else {
                    mouse.release(button);
                }
            }
            InputBinding::Chord { .. } => unreachable!(),
        };
        press(&mut app, true);
        app.update();
        assert_eq!(food_requests(&mut app), [Some(0)]);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(50));
        app.update();
        assert!(food_requests(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        app.update();
        assert_eq!(food_requests(&mut app), [Some(0)]);
        press(&mut app, false);
        app.update();
        assert_eq!(food_requests(&mut app), [None]);
        press(&mut app, true);
        app.update();
        assert_eq!(food_requests(&mut app), [Some(0)]);
        action(&mut app, InventoryAction::Toggle);
        assert_eq!(food_requests(&mut app), [None]);
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Prop(ResourcePropKind::Mushroom)),
            3,
        );
    }
}

#[test]
fn configured_place_on_creature_does_not_open_or_crank_background_stations_but_explicit_use_does() {
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::{
        edits::{BlockObjectState, EditJournal},
        streaming::{ChunkStore, LoadedSection},
    };
    for material in [28, 29, 59, 99, 100, terra_core::mixer::MATERIAL] {
        for (creature, explicit_use) in [(Some(3), false), (Some(3), true), (None, false)] {
            let mut app = food_input_app(InputBinding::Key(KeyCode::KeyU));
            app.init_resource::<terra_network::client::CrankInput>()
                .init_resource::<terra_player::CameraInputState>()
                .insert_resource(terra_player::TargetCreature(creature))
                .add_systems(PreUpdate, block_opening_frame)
                .add_systems(
                    Update,
                    drive_crusher.after(actions).before(held_consumption),
                );
            app.world_mut()
                .resource_mut::<terra_player::CameraInputState>()
                .captured = true;
            let owner = SectionPos::default();
            let position = [3, 1, 1];
            let mut data = BlockSection::empty(owner).unwrap();
            data.set_block([3, 1, 1], material).unwrap();
            let mut journal = EditJournal::default();
            if material == 100 {
                for (position, material, peer, secondary) in [
                    ([1, 1, 1], 99, [2, 1, 1], false),
                    ([2, 1, 1], 99, [1, 1, 1], true),
                    ([3, 1, 1], 100, [1, 1, 1], false),
                ] {
                    data.set_block(position.map(|v| v as usize), material)
                        .unwrap();
                    journal.record(position, material).unwrap();
                    journal
                        .set_object_state(
                            position,
                            Some(BlockObjectState {
                                peer: Some(peer),
                                secondary,
                                ..default()
                            }),
                        )
                        .unwrap();
                }
            }
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            app.insert_resource(chunks)
                .insert_resource(journal)
                .insert_resource(terra_player::TargetWorld(Some(
                    terra_player::WorldTarget::Block(BlockHit {
                        position,
                        normal: [1, 0, 0],
                        distance: 2.0,
                    }),
                )));
            app.world_mut()
                .resource_mut::<Messages<InventoryAction>>()
                .clear();
            let key = if explicit_use {
                KeyCode::KeyR
            } else {
                KeyCode::KeyU
            };
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            let station_allowed = creature.is_none() || explicit_use;
            let opens: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<InventoryAction>>()
                .drain()
                .filter(|action| {
                    matches!(
                        action,
                        InventoryAction::OpenChest(_) | InventoryAction::OpenWorkbench(_)
                    )
                })
                .collect();
            assert_eq!(
                !opens.is_empty(),
                station_allowed && material != 100,
                "material={material}, creature={creature:?}, explicit_use={explicit_use}"
            );
            if station_allowed && material == terra_core::mixer::MATERIAL {
                assert!(
                    matches!(opens.as_slice(), [InventoryAction::OpenChest(p)] if *p == position)
                );
                assert!(!app.world().resource::<InventoryState>().assembly);
            }
            assert_eq!(
                app.world().resource::<InventoryState>().open,
                station_allowed && material != 100
            );
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::CrankInput>()
                    .0,
                (station_allowed && material == 100).then_some(position)
            );
            assert!(
                food_requests(&mut app).is_empty(),
                "background interaction must never start eating"
            );
            assert!(
                app.world()
                    .resource::<terra_player::CameraInputState>()
                    .captured
            );
            if !station_allowed {
                assert!(!app.world().resource::<PlayerInputBlock>().0);
                assert!(
                    app.world_mut()
                        .resource_mut::<Messages<ContainerRequest>>()
                        .drain()
                        .next()
                        .is_none()
                );
                // Holding Place must not start the crank or consumption on the next frame.
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .clear_just_pressed(key);
                app.update();
                assert_eq!(
                    app.world()
                        .resource::<terra_network::client::CrankInput>()
                        .0,
                    None
                );
                assert!(food_requests(&mut app).is_empty());
                assert!(!app.world().resource::<InventoryState>().open);
            }
        }
    }
}

#[test]
fn stations_and_targeted_creature_feeding_take_priority_over_place_eating() {
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};

    for material in [28, 29, 59] {
        let mut app = food_input_app(InputBinding::Key(KeyCode::KeyU));
        let position = SectionPos::default();
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([1, 1, 1], material).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(BlockHit {
                    position: [1, 1, 1],
                    normal: [0, 1, 0],
                    distance: 2.0,
                }),
            )));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyU);
        app.update();
        assert!(app.world().resource::<InventoryState>().open);
        assert!(food_requests(&mut app).is_empty());
    }
    let mut app = food_input_app(InputBinding::Key(KeyCode::KeyU));
    app.insert_resource(terra_player::TargetCreature(Some(3)));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyU);
    app.update();
    assert!(food_requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(food_requests(&mut app), [Some(0)]);
}

#[test]
fn doors_windows_valves_and_faucets_use_place_without_opening_inventory_or_eating() {
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};
    for material in (65..=70).chain([166, 167]) {
        let mut app = food_input_app(InputBinding::Key(KeyCode::KeyU));
        app.init_resource::<PlayerInputBlock>()
            .add_systems(PreUpdate, block_opening_frame);
        let section = SectionPos::default();
        let position = [1, 1, 1];
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([1, 1, 1], material).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(BlockHit {
                    position,
                    normal: [0, 0, 1],
                    distance: 2.0,
                }),
            )));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyU);
        app.update();
        assert!(!app.world().resource::<InventoryState>().open);
        assert!(
            !app.world().resource::<PlayerInputBlock>().0,
            "Opening a door/window must preserve gameplay capture"
        );
        assert!(food_requests(&mut app).is_empty());
        let requests = app
            .world_mut()
            .resource_mut::<Messages<terra_network::client::ObjectInteractionRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].position, position);
        assert_eq!(requests[0].expected, material);
        assert_eq!(requests[0].facing, 0);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<terra_player::TargetWorld>()
            .0 = None;
        for shift in [false, true, false] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            if shift {
                keys.press(KeyCode::ShiftLeft);
            } else {
                keys.release(KeyCode::ShiftLeft);
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(500));
            app.update();
            assert!(
                food_requests(&mut app).is_empty(),
                "holding the opening action must not start eating"
            );
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyU);
        app.update();
        assert!(food_requests(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyU);
        app.update();
        assert_eq!(
            food_requests(&mut app),
            [Some(0)],
            "a fresh press after release must start eating normally"
        );
    }
}

#[test]
fn creative_r_inspects_hovered_catalog_recipes_without_granting_or_loading_items() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        InventoryAction::Hover {
            command: "catalog".into(),
            index: None,
            item: "block-16".into(),
        },
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(view.recipe_open);
    assert!(!view.recipe_output.is_empty());
    assert_eq!(view.recipe_output[0].item_id, "block-16");
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slots
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .all(|slot| slot.empty)
    );
}

#[test]
fn creative_recipe_shortcut_ignores_search_typing_and_non_catalog_slots() {
    for typing in [false, true] {
        let mut app = app(GameMode::Creative);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .creative_set(
                0,
                terra_core::ItemStack {
                    item: ItemKind::Block(16),
                    count: 1,
                },
            );
        action(&mut app, InventoryAction::Toggle);
        action(
            &mut app,
            InventoryAction::Hover {
                command: if typing { "catalog" } else { "slot" }.into(),
                index: Some(0),
                item: "block-16".into(),
            },
        );
        if typing {
            let field = app
                .world_mut()
                .spawn(ElementId("inventory-search".into()))
                .id();
            app.insert_resource(InputFocus::from_entity(field));
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert!(!app.world().resource::<crafting::CraftingView>().recipe_open);
        assert!(requests(&mut app).is_empty());
    }
}

#[test]
fn recipe_browser_works_beside_chest_but_cannot_fill_an_invisible_crafting_grid() {
    let mut app = app(GameMode::Creative);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 4);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([-6, 70, 9], false));
    app.update();
    action(&mut app, InventoryAction::Recipe("block-16".into()));
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(view.recipe_open);
    assert!(!view.can_apply_recipe);
    app.world_mut()
        .write_message(crafting::CraftingAction::ApplyRecipe);
    app.update();
    assert!(app.world().resource::<crafting::CraftingView>().recipe_open);
    assert!(
        app.world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .all(|slot| slot.empty)
    );
    assert!(requests(&mut app).is_empty());
    assert!(container_requests(&mut app).is_empty());
}

#[test]
fn catalog_capacity_tracks_the_real_grid_after_resize_and_dpi_changes() {
    let mut app = App::new();
    app.init_resource::<BlockPalette>()
        .insert_resource(InventoryState {
            open: true,
            ..default()
        })
        .add_systems(Update, browser_layout);
    let grid = app
        .world_mut()
        .spawn((
            ElementId("inventory-browser-grid".into()),
            ComputedNode {
                size: Vec2::splat(605.0),
                ..default()
            },
            Node {
                column_gap: Val::Px(5.0),
                row_gap: Val::Px(5.0),
                ..default()
            },
        ))
        .id();
    let cell = app
        .world_mut()
        .spawn((
            ElementId("inventory-cell".into()),
            ComputedNode {
                size: Vec2::splat(56.0),
                ..default()
            },
            Node::default(),
            ChildOf(grid),
        ))
        .id();
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().page_capacity, 100);
    app.world_mut()
        .get_mut::<ComputedNode>(grid)
        .unwrap()
        .size
        .y = 300.0;
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().page_capacity, 50);
    {
        let mut node = app.world_mut().get_mut::<ComputedNode>(grid).unwrap();
        node.size = Vec2::splat(1210.0);
        node.inverse_scale_factor = 0.5;
    }
    app.world_mut().get_mut::<ComputedNode>(cell).unwrap().size = Vec2::splat(112.0);
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().page_capacity, 100);
}

#[test]
fn catalog_has_all_first_and_keeps_scrollable_tabs_at_every_width() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    assert_eq!(
        app.world().resource::<InventoryView>().categories[0].id,
        "all"
    );
    assert_eq!(
        app.world()
            .resource::<InventoryView>()
            .categories
            .iter()
            .filter(|c| c.id == "all")
            .count(),
        1
    );
    app.add_systems(PostUpdate, browser_layout);
    let host = app
        .world_mut()
        .spawn((
            ElementId("inventory-categories-host".into()),
            ComputedNode {
                size: Vec2::new(200.0, 80.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            Node::default(),
        ))
        .id();
    app.update();
    assert!(!app.world().resource::<InventoryState>().category_dropdown);
    app.world_mut()
        .get_mut::<ComputedNode>(host)
        .unwrap()
        .size
        .x = 900.0;
    app.update();
    assert!(!app.world().resource::<InventoryState>().category_dropdown);
    action(&mut app, InventoryAction::Category("wood".into()));
    assert!(
        app.world()
            .resource::<InventoryView>()
            .browser_items
            .iter()
            .all(|i| i.category == "wood")
    );
    action(&mut app, InventoryAction::Category("all".into()));
    assert!(
        app.world()
            .resource::<InventoryView>()
            .browser_items
            .iter()
            .any(|i| i.category != "wood")
    );
}

#[test]
fn an_open_inventory_waits_for_stable_layout_and_resets_after_closing() {
    let mut app = App::new();
    app.insert_resource(InventoryView {
        open: true,
        layout_style: "opacity: 0; pointer-events: none;".into(),
        ..default()
    })
    .add_systems(Update, reveal_layout);
    app.world_mut().spawn((
        ElementId("inventory-panel".into()),
        ComputedNode {
            size: Vec2::new(704.0, 684.0),
            ..default()
        },
    ));
    for _ in 0..2 {
        app.update();
        assert!(
            app.world()
                .resource::<InventoryView>()
                .layout_style
                .contains("opacity: 0")
        );
    }
    app.update();
    assert_eq!(
        app.world().resource::<InventoryView>().layout_style,
        "opacity: 1; pointer-events: auto;"
    );
    app.world_mut().resource_mut::<InventoryView>().open = false;
    app.update();
    {
        let mut view = app.world_mut().resource_mut::<InventoryView>();
        view.open = true;
        view.layout_style = "opacity: 0;".into();
    }
    app.update();
    assert_eq!(
        app.world().resource::<InventoryView>().layout_style,
        "opacity: 0;"
    );
}

#[test]
fn hidden_inventory_prewarms_low_cadence_browser_snapshot_without_revealing_or_capturing_input() {
    let mut app = App::new();
    app.insert_resource(InventoryView {
        visible: true,
        prewarming: true,
        layout_style: "opacity: 0; pointer-events: none;".into(),
        ..default()
    })
    .init_resource::<InventoryState>()
    .init_resource::<PlayerInputBlock>()
    .add_systems(Update, reveal_layout);
    app.world_mut().spawn((
        ElementId("inventory-panel".into()),
        ComputedNode {
            size: Vec2::new(704.0, 840.0),
            ..default()
        },
    ));
    for _ in 0..5 {
        app.update();
        assert!(app.world().resource::<InventoryView>().prewarming);
    }
    app.world_mut()
        .resource_mut::<InventoryState>()
        .page_capacity = 45;
    app.world_mut()
        .resource_mut::<InventoryView>()
        .browser_capacity = 45;
    app.update();
    app.update();
    // A changed capacity requires its own settled materialization, not stale first-frame slots.
    app.world_mut()
        .resource_mut::<InventoryState>()
        .page_capacity = 56;
    // Simulate the serialized view waiting for a low-frequency UI cadence tick.
    for _ in 0..8 {
        app.update();
        assert!(app.world().resource::<InventoryView>().prewarming);
    }
    app.world_mut()
        .resource_mut::<InventoryView>()
        .browser_capacity = 56;
    app.world_mut()
        .resource_mut::<InventoryState>()
        .category_dropdown = true;
    for _ in 0..5 {
        app.update();
        assert!(app.world().resource::<InventoryView>().prewarming);
    }
    app.world_mut()
        .resource_mut::<InventoryView>()
        .category_dropdown = true;
    for _ in 0..2 {
        app.update();
        assert!(app.world().resource::<InventoryView>().prewarming);
    }
    app.update();
    let view = app.world().resource::<InventoryView>();
    assert!(!view.prewarming);
    assert!(!view.open);
    assert!(view.layout_style.contains("opacity: 0"));
    assert!(!app.world().resource::<PlayerInputBlock>().0);
}

#[test]
fn container_opening_never_reveals_the_player_layout_before_its_server_reply() {
    let mut app = App::new();
    app.insert_resource(InventoryView {
        open: true,
        layout_style: "opacity: 0;".into(),
        ..default()
    })
    .insert_resource(InventoryState {
        open: true,
        container_open_response: Some(0),
        ..default()
    })
    .add_systems(Update, reveal_layout);
    app.world_mut().spawn((
        ElementId("inventory-panel".into()),
        ComputedNode {
            size: Vec2::new(704.0, 684.0),
            ..default()
        },
    ));
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<InventoryView>().layout_style,
        "opacity: 0;"
    );
    app.world_mut()
        .resource_mut::<InventoryState>()
        .container_open_response = None;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<InventoryView>().layout_style,
        "opacity: 1; pointer-events: auto;"
    );
}

#[test]
fn inventory_stylesheets_use_supported_tilt_ui_properties() {
    tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
        "../../../src-ui/styles/inventory-reference.css"
    ))
    .unwrap();
    for source in [
        include_str!("../../../src-ui/components/inventory-panel.component.css"),
        include_str!("../../../src-ui/components/inventory-slot.component.css"),
        include_str!("../../../src-ui/components/inventory-category.component.css"),
        include_str!("../../../src-ui/components/equipment-slot.component.css"),
        include_str!("../../../src-ui/components/crafting-panel.component.css"),
        include_str!("../../../src-ui/components/item-browser.component.css"),
        include_str!("../../../src-ui/components/recipe-inspector.component.css"),
    ] {
        tilt_ui::tilt_ui_css::parse_stylesheet(source).unwrap();
    }
}

#[test]
fn inventory_templates_load_with_only_builtin_or_registered_tilt_ui_elements() {
    use bevy::asset::{AssetPlugin, AssetServer, Assets};
    let mut app = App::new();
    app.add_plugins(bevy::app::TaskPoolPlugin::default())
        .add_plugins(tilt_ui::TiltUiAssetSourcePlugin::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-ui"),
        ))
        .add_plugins(AssetPlugin::default())
        .add_plugins(tilt_ui::TiltUiAssetsPlugin);
    let templates = [
        "equipment-slot",
        "inventory-panel",
        "inventory-slot",
        "inventory-category",
        "crafting-panel",
        "item-browser",
        "recipe-inspector",
        "train-workshop",
    ]
    .map(|name| {
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<tilt_ui::UiTemplateAsset>(format!(
                "tilt-ui://components/{name}.component.html"
            ));
        (name, handle)
    });
    for _ in 0..500 {
        app.update();
        if templates.iter().all(|(_, handle)| {
            app.world()
                .resource::<Assets<tilt_ui::UiTemplateAsset>>()
                .get(handle)
                .is_some()
        }) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let assets = app.world().resource::<Assets<tilt_ui::UiTemplateAsset>>();
    for (name, handle) in templates {
        let template = assets
            .get(&handle)
            .unwrap_or_else(|| panic!("TiltUI template did not load: {name}"));
        for node in template.template().nodes() {
            if let tilt_ui::TemplateNodeKind::Component(component) = &node.kind {
                assert!(
                    component.as_str().starts_with("tilt-flow-")
                        || crate::TILT_UI_COMPONENTS
                            .iter()
                            .any(|known| known.name == component.as_str()),
                    "Unknown TiltUI component {component} in {name}",
                );
            }
        }
    }
}

#[test]
fn tooltips_escape_the_scroll_clip_without_unclipping_inventory_slots() {
    let mut app = App::new();
    app.add_systems(Update, unclip_slot_tooltips);
    let tooltip = app
        .world_mut()
        .spawn(ElementId("inventory-tooltip".into()))
        .id();
    let slot = app
        .world_mut()
        .spawn(ElementId("inventory-cell".into()))
        .id();
    let category = app
        .world_mut()
        .spawn(ElementId("inventory-category-tooltip".into()))
        .id();
    let unnamed = app
        .world_mut()
        .spawn(tilt_ui::TiltElement {
            kind: tilt_ui::ElementKind::ToolTip,
        })
        .id();
    app.update();
    assert!(app.world().get::<bevy::ui::OverrideClip>(tooltip).is_some());
    assert!(
        app.world()
            .get::<bevy::ui::OverrideClip>(category)
            .is_some()
    );
    assert!(app.world().get::<bevy::ui::OverrideClip>(slot).is_none());
    assert!(app.world().get::<bevy::ui::OverrideClip>(unnamed).is_some());
}

fn app(mode: GameMode) -> App {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ControlsConfig>()
        .init_resource::<InventoryState>()
        .init_resource::<InventoryView>()
        .init_resource::<crate::SettingsView>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<ActiveItem>()
        .init_resource::<SelectedInventorySlot>()
        .init_resource::<InputFocus>()
        .init_resource::<Time>()
        .init_resource::<pointer::InventoryPointer>()
        .init_resource::<pointer::InventoryQueue>()
        .init_resource::<ClientSession>()
        .insert_resource(MenuState {
            page: MenuPage::Playing,
            ..default()
        })
        .insert_resource(BlockPalette {
            item_metadata: default(),
            choices: [
                (3, "Stone"),
                (2, "Dirt"),
                (4, "Sand"),
                (6, "Oak Wood"),
                (16, "Oak Planks"),
            ]
            .into_iter()
            .map(|(id, name)| BlockChoice {
                id,
                name: name.into(),
                identifier: ItemKind::Block(id).identifier(),
                description: format!("A test description for {name}."),
            })
            .collect(),
            selected: 0,
            prop_descriptions: default(),
            creative_categories: [
                ("all", "All", ItemKind::Block(3)),
                ("terrain", "Terrain", ItemKind::Block(3)),
                ("wood", "Wood", ItemKind::Block(6)),
                ("nature", "Nature", ItemKind::Prop(ResourcePropKind::Flower)),
                ("building", "Building", ItemKind::Block(16)),
            ]
            .into_iter()
            .enumerate()
            .map(
                |(order, (id, name, icon))| terra_player::interaction::CreativeCategory {
                    id: id.into(),
                    name: name.into(),
                    icon,
                    order: order as i32,
                },
            )
            .collect(),
            item_categories: [
                (ItemKind::Block(3), "terrain"),
                (ItemKind::Block(2), "terrain"),
                (ItemKind::Block(4), "terrain"),
                (ItemKind::Block(6), "wood"),
                (ItemKind::Block(16), "wood"),
            ]
            .into_iter()
            .chain(prop_kinds().map(|kind| (ItemKind::Prop(kind), "nature")))
            .map(|(item, category)| (item, category.into()))
            .collect(),
        })
        .add_message::<CaptureRequest>()
        .add_message::<InventoryAction>()
        .add_message::<InventoryRequest>()
        .add_message::<ContainerRequest>()
        .add_message::<terra_network::client::ObjectInteractionRequest>()
        .add_message::<MouseWheel>()
        .add_systems(
            Update,
            (
                keyboard,
                actions,
                pointer::finish_gesture,
                pointer::send_requests,
                hotbar_wheel,
                view::update_view,
            )
                .chain()
                .after(crate::menu::MenuInputSet),
        );
    crafting::install(&mut app);
    app.insert_resource(terra_core::crafting::RecipeBook::builtins().clone());
    let mut session = app.world_mut().resource_mut::<ClientSession>();
    session.ready = true;
    session.game_mode = mode;
    app
}

fn action(app: &mut App, action: InventoryAction) {
    app.world_mut().write_message(action);
    app.update();
}
fn requests(app: &mut App) -> Vec<InventoryCommand> {
    app.world_mut()
        .resource_mut::<Messages<InventoryRequest>>()
        .drain()
        .map(|request| request.0)
        .collect()
}

#[test]
fn creative_trash_targets_real_selected_stacks_and_prefers_the_cursor() {
    for held in [false, true] {
        let mut app = app(GameMode::Creative);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.inventory.creative_set(
                12,
                terra_core::ItemStack {
                    item: ItemKind::Block(3),
                    count: 41,
                },
            );
            if held {
                session.inventory.creative_cursor(terra_core::ItemStack {
                    item: ItemKind::Block(2),
                    count: 19,
                });
            }
        }
        action(&mut app, InventoryAction::Toggle);
        action(&mut app, InventoryAction::Slot(12));
        assert!(app.world().resource::<InventoryView>().trash_available);
        let before = app.world().resource::<ClientSession>().inventory.clone();
        action(&mut app, InventoryAction::Trash);
        assert_eq!(
            requests(&mut app)[0].action,
            ServerAction::CreativeTrash {
                slot: if held { None } else { Some(12) },
            }
        );
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    }
}

#[test]
fn control_trash_clears_the_whole_creative_bag_through_authority_only() {
    for key in [KeyCode::ControlLeft, KeyCode::ControlRight] {
        let mut app = app(GameMode::Creative);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            assert!(session.inventory.creative_set(
                terra_core::INVENTORY_SLOTS - 1,
                terra_core::ItemStack {
                    item: ItemKind::Block(6),
                    count: 41,
                },
            ));
            session.stats.equipment.set(
                terra_core::EquipmentSlot::Helmet,
                Some(ItemKind::Armor(terra_core::ArmorItem {
                    material: terra_core::ArmorMaterial::Iron,
                    piece: terra_core::ArmorPiece::Helmet,
                })),
            );
        }
        action(&mut app, InventoryAction::Toggle);
        app.world_mut()
            .write_message(crafting::CraftingAction::Search("oak_planks".into()));
        app.update();
        add_recipe(&mut app);
        assert!(
            app.world()
                .resource::<crafting::CraftingView>()
                .grid
                .iter()
                .any(|slot| !slot.empty)
        );
        assert!(
            app.world_mut()
                .resource_mut::<ClientSession>()
                .inventory
                .creative_cursor(terra_core::ItemStack {
                    item: ItemKind::Block(2),
                    count: 19
                })
        );
        let before = app.world().resource::<ClientSession>().inventory.clone();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        action(&mut app, InventoryAction::Trash);
        assert_eq!(requests(&mut app)[0].action, ServerAction::CreativeClear);
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.inventory = terra_core::Inventory {
                revision: before.revision + 1,
                ..default()
            };
            session.stats.equipment = default();
            session.inventory_response += 1;
            session.last_inventory =
                Some(terra_network::protocol::inventory::InventoryOutcome::Applied);
        }
        app.update();
        assert!(
            app.world()
                .resource::<crafting::CraftingView>()
                .grid
                .iter()
                .all(|slot| slot.empty)
        );
        assert!(!app.world().resource::<InventoryView>().trash_available);
    }
}

#[test]
fn creative_trash_ignores_empty_targets_closed_inventory_and_other_modes() {
    for mode in [GameMode::Creative, GameMode::Survival, GameMode::Spectator] {
        let mut app = app(mode);
        action(&mut app, InventoryAction::Toggle);
        assert!(!app.world().resource::<InventoryView>().trash_available);
        action(&mut app, InventoryAction::Trash);
        assert!(requests(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .insert(ItemKind::Block(3), 4);
        app.update();
        assert_eq!(
            app.world().resource::<InventoryView>().trash_available,
            mode == GameMode::Creative
        );
        if mode != GameMode::Creative {
            action(&mut app, InventoryAction::Trash);
            assert!(requests(&mut app).is_empty());
        }
        action(&mut app, InventoryAction::Close);
        requests(&mut app);
        action(&mut app, InventoryAction::Trash);
        assert!(requests(&mut app).is_empty());
    }
    let template = include_str!("../../../src-ui/components/inventory-panel.component.html");
    let browser = include_str!("../../../src-ui/components/item-browser.component.html");
    assert!(!template.contains("id=\"inventory-trash\""));
    assert!(browser.contains("id=\"inventory-trash\""));
    assert!(browser.contains("!inventoryView.creative || !inventoryView.trash_available"));
    assert!(!template.contains("inventory-cancel-move"));
    assert!(!template.contains("inventoryView.carried"));
}

fn add_recipe(app: &mut App) {
    app.world_mut()
        .write_message(crafting::CraftingAction::ApplyRecipe);
    app.update();
}

fn hover_craft_cell(app: &mut App, index: u8, item: &str) {
    action(
        app,
        InventoryAction::Hover {
            command: "craft-cell".into(),
            index: Some(index),
            item: item.into(),
        },
    );
}

#[test]
fn crafting_left_drag_distributes_on_release_and_shares_backing_without_duplication() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(6),
        count: 11,
    });
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(&mut app, pointer_action("craft-cell", Some(0), "", false));
    hover_craft_cell(&mut app, 1, "");
    hover_craft_cell(&mut app, 2, "");
    assert!(requests(&mut app).is_empty());
    for index in [0, 1, 2] {
        let slot = app
            .world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .find(|slot| slot.index == index.to_string())
            .unwrap();
        assert!(!slot.empty);
        assert_eq!(slot.count, "3");
    }
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .map(|slot| slot.count.as_str())
            .collect::<Vec<_>>(),
        ["3", "3", "3", ""]
    );
    assert_eq!(
        crafting::reserved_counts(app.world().resource::<crafting::CraftingState>()),
        [0; terra_core::INVENTORY_SLOTS]
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .unwrap()
            .count,
        11
    );
    let command = apply_pointer_ack(&mut app);
    assert!(matches!(
        command,
        ServerAction::TransferCursor {
            to_cursor: false,
            ..
        }
    ));
    let view = app.world().resource::<crafting::CraftingView>();
    assert_eq!(
        view.grid
            .iter()
            .map(|s| s.count.as_str())
            .collect::<Vec<_>>(),
        ["3", "3", "3", ""]
    );
    let bag = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(bag.cursor.unwrap().count, 2);
    assert_eq!(bag.slot(0).unwrap().count, 9);
    assert_eq!(
        bag.count(ItemKind::Block(6)) + bag.cursor.unwrap().count,
        11
    );
    assert!(app.world().resource::<InventoryView>().hotbar[0].empty);
}

#[test]
fn crafting_right_sweep_keeps_visits_while_waiting_and_places_one_per_entry() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(6),
        count: 8,
    });
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    action(&mut app, pointer_action("craft-cell", Some(0), "", true));
    let initial = requests(&mut app);
    assert_eq!(initial.len(), 1);
    hover_craft_cell(&mut app, 1, "");
    hover_craft_cell(&mut app, 2, "");
    apply_pointer_command(&mut app, initial[0]);
    for _ in 0..2 {
        app.update();
        apply_pointer_ack(&mut app);
    }
    assert_eq!(
        app.world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .map(|s| s.count.as_str())
            .collect::<Vec<_>>(),
        ["1", "1", "1", ""]
    );
    action(
        &mut app,
        InventoryAction::Leave {
            command: "craft-cell".into(),
            index: Some(2),
            item: "block-6".into(),
        },
    );
    hover_craft_cell(&mut app, 1, "block-6");
    apply_pointer_ack(&mut app);
    let bag = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(bag.cursor.unwrap().count, 4);
    assert_eq!(bag.count(ItemKind::Block(6)), 4);
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[1].count,
        "2"
    );
}

#[test]
fn crafting_wheel_transfers_exact_items_both_directions_without_touching_other_cells() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(6),
        count: 4,
    });
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::CraftCell(0));
    apply_pointer_ack(&mut app);
    hover_craft_cell(&mut app, 0, "block-6");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    for delta in [2.0, -2.0] {
        app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: delta,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        for _ in 0..2 {
            app.update();
            apply_pointer_ack(&mut app);
        }
        assert!(
            app.world().resource::<crafting::CraftingView>().grid[1..]
                .iter()
                .all(|s| s.empty)
        );
        let bag = &app.world().resource::<ClientSession>().inventory;
        assert_eq!(
            bag.count(ItemKind::Block(6)) + bag.cursor.map_or(0, |s| s.count),
            4
        );
    }
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "4"
    );
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
}

#[test]
fn crafting_shift_sweep_releases_cells_to_backpack_without_moving_or_duplicating_items() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .choices
        .push(BlockChoice {
            id: 28,
            name: "Workbench".into(),
            identifier: "teramic::workbench".into(),
            description: "Workbench".into(),
        });
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(16), 6);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("catalog", None, "block-28", false));
    add_recipe(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(
        &mut app,
        pointer_action("craft-cell", Some(0), "block-16", false),
    );
    hover_craft_cell(&mut app, 1, "block-16");
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(view.grid[0].empty && view.grid[1].empty);
    assert_eq!(view.grid[2].count, "1");
    assert_eq!(view.grid[3].count, "1");
    assert!(requests(&mut app).is_empty());
    assert_eq!(app.world().resource::<InventoryView>().hotbar[0].count, "4");
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(16)),
        6
    );
}

#[test]
fn recipe_preview_and_alternatives_leave_existing_grid_untouched_until_plus() {
    use terra_core::crafting::{RecipeBook, recipe_key};
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 5);
    {
        let mut book = app.world_mut().resource_mut::<RecipeBook>();
        let mut recipe = book.get(recipe_key("teramic::oak_planks")).unwrap().clone();
        recipe.id = "test::alternative".into();
        recipe.name = "Z Alternative".into();
        book.extend(vec![recipe]).unwrap();
    }
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Slot(0));
    action(&mut app, InventoryAction::CraftCell(3));
    let before = app
        .world()
        .resource::<crafting::CraftingView>()
        .grid
        .clone();
    action(&mut app, pointer_action("catalog", None, "block-16", false));
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid,
        before
    );
    assert!(
        app.world()
            .resource::<crafting::CraftingView>()
            .recipe_multiple
    );
    app.world_mut()
        .write_message(crafting::CraftingAction::Alternative(1));
    app.update();
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid,
        before
    );
    add_recipe(&mut app);
    assert!(!app.world().resource::<crafting::CraftingView>().recipe_open);
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
    assert!(app.world().resource::<crafting::CraftingView>().grid[3].empty);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(6)),
        5
    );
}

#[test]
fn recipe_search_previews_then_explicitly_adds_reserved_counts_without_local_inventory_mutation() {
    use super::crafting::{CraftingAction, CraftingView};
    use terra_core::crafting::{CraftDestination, CraftGridCell};
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 3);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Assembly(true));
    app.world_mut()
        .write_message(CraftingAction::Search("oak_planks".into()));
    app.update();
    let view = app.world().resource::<CraftingView>();
    assert!(view.recipe_open && view.selected);
    assert_eq!(view.name, "Oak Planks");
    assert!(view.grid.iter().all(|slot| slot.empty));
    assert!(view.can_apply_recipe && !view.recipe_multiple);
    add_recipe(&mut app);
    let view = app.world().resource::<CraftingView>();
    assert!(!view.recipe_open);
    assert_eq!(view.output[0].preview_key, "Block preview 16");
    assert_eq!(view.grid[0].count, "1");
    assert_eq!(view.maximum, "1");
    app.world_mut().write_message(CraftingAction::Craft {
        maximum: true,
        cursor: false,
    });
    app.update();
    let commands = requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].action,
        ServerAction::CraftReserved {
            cells: [
                Some(CraftGridCell { slot: 0, count: 1 }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None
            ],
            size: 2,
            count: 1,
            station: None,
            destination: CraftDestination::Inventory,
        }
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(6)),
        3
    );
}

#[test]
fn pending_crafts_keep_their_grid_when_recipe_selection_navigation_or_clear_is_attempted() {
    use crafting::{CraftingAction, CraftingView};
    use terra_core::crafting::{RecipeBook, recipe_key};

    for event in [
        CraftingAction::Select(recipe_key("teramic::birch_planks")),
        CraftingAction::Search("teramic::birch_planks".into()),
        CraftingAction::Alternative(1),
        CraftingAction::Clear,
    ] {
        let mut app = app(GameMode::Survival);
        {
            let mut book = app.world_mut().resource_mut::<RecipeBook>();
            let mut alternative = book.get(recipe_key("teramic::oak_planks")).unwrap().clone();
            alternative.id = "test::alternative_oak_planks".into();
            alternative.name = "Z Alternative Oak Planks".into();
            book.extend(vec![alternative]).unwrap();
        }
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.inventory.insert(ItemKind::Block(6), 3);
            session.inventory.insert(ItemKind::Block(10), 2);
        }
        action(&mut app, InventoryAction::Toggle);
        action(&mut app, pointer_action("catalog", None, "block-16", false));
        assert_eq!(app.world().resource::<CraftingView>().name, "Oak Planks");
        assert!(app.world().resource::<CraftingView>().recipe_next);
        assert!(app.world().resource::<CraftingView>().recipe_multiple);
        add_recipe(&mut app);
        action(&mut app, InventoryAction::Craft);
        let pending_command = requests(&mut app)[0];
        app.world_mut().write_message(event);
        app.update();
        let view = app.world().resource::<CraftingView>();
        assert_eq!(view.name, "Oak Planks");
        assert_eq!(view.grid[0].item_id, "block-6");
        assert_eq!(view.grid[0].count, "1");
        assert!(view.message.contains("Waiting"));
        action(&mut app, pointer_action("catalog", None, "block-3", false));
        assert_eq!(app.world().resource::<CraftingView>().name, "Oak Planks");
        app.world_mut().write_message(CraftingAction::CloseRecipe);
        app.update();
        assert!(!app.world().resource::<CraftingView>().recipe_open);
        assert!(matches!(
            apply_pointer_command(&mut app, pending_command),
            ServerAction::CraftReserved { .. }
        ));
        let view = app.world().resource::<CraftingView>();
        assert!(view.grid.iter().all(|slot| slot.empty));
        assert!(view.message.is_empty());
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(6)),
            2
        );
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(16)),
            4
        );
        app.world_mut()
            .write_message(CraftingAction::Select(recipe_key("teramic::birch_planks")));
        app.update();
        assert_eq!(app.world().resource::<CraftingView>().name, "Birch Planks");
    }
}

#[test]
fn recipe_browser_shows_station_and_missing_errors_and_never_sends_invalid_crafts() {
    use super::crafting::{CraftingAction, CraftingView};
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(16), 128);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Assembly(true));
    app.world_mut()
        .write_message(CraftingAction::Search("teramic::chest".into()));
    app.update();
    assert!(!app.world().resource::<CraftingView>().enabled);
    assert!(
        app.world()
            .resource::<CraftingView>()
            .message
            .contains("workbench")
    );
    app.world_mut().write_message(CraftingAction::Craft {
        maximum: false,
        cursor: false,
    });
    app.update();
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .write_message(CraftingAction::Search("teramic::oak_planks".into()));
    app.update();
    assert!(
        app.world()
            .resource::<CraftingView>()
            .message
            .contains("Missing")
    );
    assert!(app.world().resource::<CraftingView>().missing);
    assert!(
        app.world()
            .resource::<CraftingView>()
            .recipe_grid
            .iter()
            .any(|slot| slot.missing)
    );
    let before = {
        let view = app.world().resource::<CraftingView>();
        (
            view.name.clone(),
            view.message.clone(),
            view.recipe_grid.clone(),
        )
    };
    // A recipe-less catalog item preserves the valid recipe already being inspected.
    action(&mut app, pointer_action("catalog", None, "block-3", false));
    let view = app.world().resource::<CraftingView>();
    assert!(view.recipe_open && view.selected && !view.empty && view.missing);
    assert!(
        (
            view.name.clone(),
            view.message.clone(),
            view.recipe_grid.clone()
        ) == before
    );
    assert!(requests(&mut app).is_empty());
}

#[test]
fn crusher_recipes_preview_and_plus_load_only_the_matching_station() {
    use super::crafting::{CraftingAction, CraftingView};
    let recipe = terra_core::crusher::recipes()
        .iter()
        .find(|recipe| matches!(recipe.output.item, ItemKind::Dust(_)))
        .unwrap();
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(recipe.input, 3);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .write_message(CraftingAction::Search(recipe.key.clone()));
    app.update();
    let view = app.world().resource::<CraftingView>();
    assert!(view.recipe_open && view.crushing && !view.smelting);
    assert_eq!(view.recipe_station, "Crusher");
    assert!(!view.can_apply_recipe);
    add_recipe(&mut app);
    assert!(requests(&mut app).is_empty());
    let anchor = [-1, 60, 2];
    app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
        anchor,
        workbench: None,
        view: ContainerView::Primary,
        attached_campfire: false,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        furnace: None,
        crusher: Some(terra_core::crusher::CrusherView::default()),
        mixer: None,
        filter: None,
        distributor: None,
        halves: vec![ContainerHalf {
            position: anchor,
            inventory: default(),
        }],
    });
    action(
        &mut app,
        pointer_action("catalog", None, &item_id(recipe.output.item), false),
    );
    assert!(app.world().resource::<CraftingView>().can_apply_recipe);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    add_recipe(&mut app);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::LoadCrushingRecipe {
            recipe: recipe.id,
            station: anchor
        }
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert!(!app.world().resource::<CraftingView>().recipe_open);
}

#[test]
fn crusher_progress_updates_without_bag_edits_and_exposes_two_correct_slots() {
    let mut app = app(GameMode::Survival);
    let anchor = [4, 60, -8];
    app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
        anchor,
        workbench: None,
        view: ContainerView::Primary,
        attached_campfire: false,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        furnace: None,
        crusher: Some(terra_core::crusher::CrusherView {
            progress: 0.25,
            working: true,
            ..default()
        }),
        mixer: None,
        filter: None,
        distributor: None,
        halves: vec![ContainerHalf {
            position: anchor,
            inventory: default(),
        }],
    });
    app.update();
    let view = app.world().resource::<InventoryView>();
    assert!(view.crusher && !view.furnace && !view.container);
    assert_eq!(view.title, "Crusher");
    assert_eq!(view.container_items.len(), 2);
    assert_eq!(view.crusher_input[0].index, "0");
    assert_eq!(view.crusher_output[0].index, "1");
    assert_eq!(view.crusher_progress_style, "width: 25.0%;");
    app.world_mut()
        .resource_mut::<ClientSession>()
        .container
        .as_mut()
        .unwrap()
        .crusher
        .as_mut()
        .unwrap()
        .progress = 0.75;
    app.update();
    assert_eq!(
        app.world()
            .resource::<InventoryView>()
            .crusher_progress_style,
        "width: 75.0%;"
    );
}

#[test]
fn all_dusts_and_flower_dyes_have_crushing_alternatives() {
    use super::crafting::{CraftingAction, CraftingView};
    for recipe in terra_core::crusher::recipes() {
        let mut app = app(GameMode::Creative);
        action(&mut app, InventoryAction::Toggle);
        app.world_mut()
            .write_message(CraftingAction::Search(recipe.key.clone()));
        app.update();
        let view = app.world().resource::<CraftingView>();
        assert!(view.crushing && view.selected, "{}", recipe.key);
        assert_eq!(view.recipe_output[0].item_id, item_id(recipe.output.item));
        assert!(!view.smelting);
    }
}

#[test]
fn smelting_recipe_browser_requires_furnace_and_loads_material_without_local_mutation() {
    use super::crafting::{CraftingAction, CraftingView};
    use terra_core::minerals::MineralItem;
    let output = ItemKind::Mineral(MineralItem::IronIngot);
    let recipe = terra_core::furnace::recipes()
        .iter()
        .find(|recipe| recipe.output.item == output)
        .unwrap();
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(recipe.input, 4);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .write_message(CraftingAction::Search(recipe.key.clone()));
    app.update();
    let view = app.world().resource::<CraftingView>();
    assert!(view.recipe_open && view.smelting && view.selected);
    assert_eq!(view.recipe_station, "Furnace");
    assert_eq!(view.recipe_grid.len(), 1);
    assert_eq!(view.recipe_output[0].item_id, item_id(output));
    assert!(!view.can_apply_recipe);
    assert!(view.message.contains("Requires a furnace"));
    add_recipe(&mut app);
    assert!(requests(&mut app).is_empty());

    let anchor = [1, 60, 1];
    app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
        anchor,
        workbench: None,
        view: ContainerView::Primary,
        attached_campfire: false,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        furnace: Some(terra_core::furnace::FurnaceView {
            progress: 0.0,
            burn_remaining: 0.0,
            burn_total: 0.0,
            recipe: None,
            smoke_cells: 0,
            vented: false,
        }),
        crusher: None,
        mixer: None,
        filter: None,
        distributor: None,
        halves: vec![ContainerHalf {
            position: anchor,
            inventory: default(),
        }],
    });
    action(
        &mut app,
        pointer_action("catalog", None, &item_id(output), false),
    );
    let view = app.world().resource::<CraftingView>();
    assert!(view.smelting && view.can_apply_recipe);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    add_recipe(&mut app);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::LoadSmeltingRecipe {
            recipe: recipe.id,
            station: anchor,
        }
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert!(!app.world().resource::<CraftingView>().recipe_open);
}

#[test]
fn smelting_recipe_plus_rejects_a_different_material_in_the_furnace() {
    use super::crafting::{CraftingAction, CraftingView};
    let recipe = terra_core::furnace::recipes().first().unwrap();
    let mut app = app(GameMode::Survival);
    let anchor = [1, 60, 1];
    let mut furnace_inventory = terra_core::Inventory::default();
    furnace_inventory.insert(ItemKind::Stick, 1);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.insert(recipe.input, 4);
        session.container = Some(ContainerSnapshot {
            anchor,
            workbench: None,
            view: ContainerView::Primary,
            attached_campfire: false,
            cauldron: None,
            campfire: None,
            tunnel_random: None,
            furnace: Some(terra_core::furnace::FurnaceView {
                progress: 0.0,
                burn_remaining: 0.0,
                burn_total: 0.0,
                recipe: None,
                smoke_cells: 0,
                vented: false,
            }),
            crusher: None,
            mixer: None,
            filter: None,
            distributor: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: furnace_inventory,
            }],
        });
    }
    app.world_mut()
        .write_message(CraftingAction::Search(recipe.key.clone()));
    app.update();
    assert!(!app.world().resource::<CraftingView>().can_apply_recipe);
    add_recipe(&mut app);
    assert!(requests(&mut app).is_empty());
}

#[test]
fn survival_browser_adds_fitting_recipes_only_after_plus_and_consumes_after_server_ack() {
    use terra_core::crafting::{CraftDestination, RecipeBook};
    use terra_network::protocol::inventory::InventoryOutcome;

    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 3);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    action(&mut app, InventoryAction::Toggle);
    assert!(
        app.world()
            .resource::<InventoryView>()
            .browser_items
            .iter()
            .any(|slot| slot.item_id == "block-16")
    );
    action(&mut app, pointer_action("catalog", None, "block-16", false));
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(view.recipe_open && view.can_apply_recipe && !view.enabled && !view.missing);
    assert!(view.grid.iter().all(|slot| slot.empty));
    assert_eq!(app.world().resource::<InventoryView>().hotbar[0].count, "3");
    add_recipe(&mut app);
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(!view.recipe_open && view.enabled);
    assert_eq!(view.grid.len(), 4);
    assert_eq!(view.grid[0].item_id, "block-6");
    assert_eq!(view.grid[0].count, "1");
    assert_eq!(app.world().resource::<InventoryView>().hotbar[0].count, "2");
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert!(requests(&mut app).is_empty());
    action(&mut app, pointer_action("craft", None, "block-16", false));
    let commands = requests(&mut app);
    let [command] = commands.as_slice() else {
        panic!("Expected one authoritative craft request")
    };
    let ServerAction::CraftReserved {
        cells,
        size,
        station,
        count,
        destination,
    } = command.action
    else {
        panic!("Expected a reserved-grid craft")
    };
    assert_eq!(size, 2);
    assert_eq!(station, None);
    assert_eq!(count, 1);
    assert_eq!(destination, CraftDestination::Cursor);
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    let book = app.world().resource::<RecipeBook>().clone();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        assert!(book.craft_reserved_grid(
            &mut session.inventory,
            &cells[..4],
            2,
            terra_core::crafting::CraftStation::Hand,
            count,
            destination
        ));
        session.inventory_response += 1;
        session.last_inventory = Some(InventoryOutcome::Applied);
    }
    app.update();
    let session = app.world().resource::<ClientSession>();
    assert_eq!(session.inventory.count(ItemKind::Block(6)), 2);
    assert_eq!(session.inventory.cursor.unwrap().item, ItemKind::Block(16));
    assert_eq!(session.inventory.cursor.unwrap().count, 4);
    assert!(
        app.world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .all(|slot| slot.empty)
    );
    assert_eq!(app.world().resource::<InventoryView>().hotbar[0].count, "2");
}

#[test]
fn survival_browser_marks_missing_ingredients_and_does_not_autofill_the_wrong_station() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .choices
        .push(BlockChoice {
            id: 29,
            name: "Chest".into(),
            identifier: ItemKind::Block(29).identifier(),
            description: "A wooden chest.".into(),
        });
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("catalog", None, "block-16", false));
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(view.recipe_open && view.missing && !view.enabled);
    assert!(
        view.recipe_grid
            .iter()
            .any(|slot| slot.missing && slot.item_id == "block-6")
    );
    assert!(view.grid.iter().all(|slot| slot.empty));
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(16), 8);
    action(&mut app, pointer_action("catalog", None, "block-29", false));
    let view = app.world().resource::<crafting::CraftingView>();
    assert!(view.recipe_open && !view.missing && !view.enabled);
    assert_eq!(view.recipe_station, "Workbench / 3 x 3");
    assert!(view.message.contains("workbench"));
    assert!(view.grid.iter().all(|slot| slot.empty));
    assert!(requests(&mut app).is_empty());
}

#[test]
fn creative_uses_the_same_backpack_browser_and_hotbar_keeps_its_real_count() {
    let mut app = app(GameMode::Creative);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 17);
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.items.len(), terra_core::INVENTORY_SLOTS - 9);
    assert_eq!(view.hotbar[0].count, "17");
    assert!(view.browser_items.iter().all(|slot| slot.count.is_empty()));
    action(&mut app, pointer_action("catalog", None, "block-16", false));
    assert!(!app.world().resource::<crafting::CraftingView>().recipe_open);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeQuickMove {
            item: ItemKind::Block(16),
            count: 1
        }
    );
}

#[test]
fn selecting_an_existing_full_creative_stack_does_not_resend_a_grant() {
    let mut app = app(GameMode::Creative);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), STACK_LIMIT);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut().resource_mut::<ActiveItem>().0 = None;
    app.world_mut().resource_mut::<SelectedInventorySlot>().0 = None;
    action(&mut app, InventoryAction::Select("block-3".into()));
    assert!(requests(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<ActiveItem>().0,
        Some(ItemKind::Block(3))
    );
    assert_eq!(app.world().resource::<SelectedInventorySlot>().0, Some(0));
}

#[test]
fn creative_catalog_contains_real_items_and_requests_a_server_stack() {
    let mut app = app(GameMode::Creative);
    let expected: Vec<_> = app
        .world()
        .resource::<BlockPalette>()
        .choices
        .iter()
        .map(|block| ItemKind::Block(block.id))
        .chain(
            ResourcePropKind::ALL
                .into_iter()
                .filter(|kind| terra_core::CropKind::from_prop(*kind).is_none())
                .map(ItemKind::Prop),
        )
        .chain(ItemKind::EXTRA)
        .collect();
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert!(view.open && view.creative && view.visible);
    assert_eq!(view.items.len(), terra_core::INVENTORY_SLOTS - 9);
    assert!(view.items.iter().all(|slot| slot.empty));
    assert_eq!(view.browser_items.len(), expected.len().min(45));
    assert_eq!(
        view.browser_page,
        format!("1 / {}", expected.len().div_ceil(45))
    );
    assert_eq!(view.browser_next, expected.len() > 45);
    assert!(!view.browser_previous);
    assert!(view.browser_items.iter().all(|item| item.count.is_empty()));
    assert!(view.detail_count.is_empty());
    assert_eq!(view.browser_items[0].preview_key, "Block preview 3");
    assert_eq!(view.browser_items[0].identifier, "teramic::stone");
    assert!(!view.browser_items[0].description.is_empty());
    assert_eq!(view.hotbar.len(), 9);
    assert!(view.hotbar.iter().all(|item| item.empty));
    let mut actual: Vec<_> = view
        .browser_items
        .iter()
        .map(|item| item.item_id.clone())
        .collect();
    let mut pages = 1;
    while app.world().resource::<InventoryView>().browser_next {
        action(&mut app, InventoryAction::Page(1));
        pages += 1;
        assert!(pages <= expected.len().div_ceil(45));
        let view = app.world().resource::<InventoryView>();
        assert!(view.browser_previous);
        assert_eq!(
            view.browser_page,
            format!("{} / {}", pages, expected.len().div_ceil(45))
        );
        assert!(view.browser_items.iter().all(|item| item.count.is_empty()));
        actual.extend(view.browser_items.iter().map(|item| item.item_id.clone()));
    }
    assert_eq!(pages, expected.len().div_ceil(45));
    let mut expected_ids: Vec<_> = expected.into_iter().map(item_id).collect();
    actual.sort();
    expected_ids.sort();
    assert_eq!(
        actual, expected_ids,
        "Every registered block, prop and extra must appear exactly once across catalog pages"
    );
    action(&mut app, InventoryAction::Select("prop-flower".into()));
    assert_eq!(
        app.world().resource::<ActiveItem>().0,
        Some(ItemKind::Prop(ResourcePropKind::Flower))
    );
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slots
            .is_empty()
    );
    let commands = requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].revision, 0);
    assert_eq!(
        commands[0].action,
        ServerAction::CreativeSet {
            slot: 0,
            item: ItemKind::Prop(ResourcePropKind::Flower),
            count: 1
        }
    );
}

#[test]
fn dye_image_identifiers_request_the_actual_item_and_hide_retired_sand_fences() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    for color in terra_core::beds::DyeColor::ALL {
        let item = ItemKind::Dye(color);
        let id = format!("item-{}_dye", color.slug());
        assert_eq!(item_id(item), id);
        action(&mut app, InventoryAction::Select(id));
        assert_eq!(app.world().resource::<ActiveItem>().0, Some(item));
    }
    assert!(!catalog_item(ItemKind::Block(95)));
    for material in 71..=73 {
        assert!(catalog_item(ItemKind::Block(material)));
        for color in terra_core::beds::DyeColor::ALL {
            assert!(!catalog_item(ItemKind::DyedBed { material, color }));
        }
    }
}

#[test]
fn search_and_categories_filter_names_and_namespaces() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Category("wood".into()));
    assert_eq!(
        app.world().resource::<InventoryView>().browser_items.len(),
        2
    );
    action(&mut app, InventoryAction::Search("planks".into()));
    assert_eq!(
        app.world().resource::<InventoryView>().browser_items[0].name,
        "Oak Planks"
    );
    action(&mut app, InventoryAction::Category("all".into()));
    action(&mut app, InventoryAction::Search("teramic::stone".into()));
    let mut actual = app
        .world()
        .resource::<InventoryView>()
        .browser_items
        .iter()
        .map(|slot| slot.identifier.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual,
        [
            "teramic::stone",
            "teramic::stone_axe",
            "teramic::stone_bellows",
            "teramic::stone_pickaxe",
            "teramic::stone_shovel",
            "teramic::stone_spoon",
            "teramic::stone_sword"
        ]
    );
    action(&mut app, InventoryAction::Search("not-an-item".into()));
    assert!(
        app.world()
            .resource::<InventoryView>()
            .browser_items
            .is_empty()
    );
}

#[test]
fn retired_castle_corner_is_absent_from_tmi_but_carried_legacy_stacks_keep_metadata() {
    use terra_core::castle;
    let mut app = app(GameMode::Creative);
    for id in [
        castle::BATTLEMENT_STRAIGHT,
        castle::BATTLEMENT_CORNER,
        castle::BATTLEMENT_PILLAR,
    ] {
        app.world_mut()
            .resource_mut::<BlockPalette>()
            .choices
            .push(BlockChoice {
                id,
                name: format!("Castle {id}"),
                identifier: format!("teramic::castle_{id}"),
                description: format!("Saved castle metadata {id}"),
            });
    }
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(castle::BATTLEMENT_CORNER), 2);
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert!(
        view.browser_items
            .iter()
            .any(|item| item.item_id == "block-180")
    );
    assert!(
        view.browser_items
            .iter()
            .any(|item| item.item_id == "block-182")
    );
    assert!(
        !view
            .browser_items
            .iter()
            .any(|item| item.item_id == "block-181")
    );
    assert_eq!(view.hotbar[0].name, "Castle 181");
    assert_eq!(view.hotbar[0].description, "Saved castle metadata 181");
    let session = app.world().resource::<ClientSession>();
    let palette = app.world().resource::<BlockPalette>();
    assert!(
        !available_items(GameMode::Creative, &session.inventory, palette)
            .contains(&ItemKind::Block(castle::BATTLEMENT_CORNER))
    );
    assert_eq!(
        inspectable_item(session, palette, "block-181"),
        Some(ItemKind::Block(castle::BATTLEMENT_CORNER))
    );
}

#[test]
fn retired_pipe_shapes_keep_inventory_metadata_but_not_catalog_entries() {
    for mode in [GameMode::Creative, GameMode::Survival] {
        let mut app = app(mode);
        for id in 60..=64 {
            app.world_mut()
                .resource_mut::<BlockPalette>()
                .choices
                .push(BlockChoice {
                    id,
                    name: format!("Named pipe {id}"),
                    identifier: format!("teramic::pipe_{id}"),
                    description: format!("Legacy pipe description {id}"),
                });
        }
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .insert(ItemKind::Block(61), 2);
        action(&mut app, InventoryAction::Toggle);
        action(
            &mut app,
            InventoryAction::Hover {
                command: "hotbar".into(),
                index: Some(0),
                item: "block-61".into(),
            },
        );
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.hotbar[0].name, "Named pipe 61");
        assert_eq!(view.hotbar[0].identifier, "teramic::pipe_61");
        assert_eq!(view.hotbar[0].description, "Legacy pipe description 61");
        assert_eq!(view.detail_name, "Named pipe 61");
        assert!(
            view.browser_items
                .iter()
                .any(|slot| slot.item_id == "block-60")
        );
        for id in 61..=64 {
            assert!(
                !view
                    .browser_items
                    .iter()
                    .any(|slot| slot.item_id == format!("block-{id}"))
            );
        }
    }
}

#[test]
fn external_namespaces_are_not_prefixed_again_and_have_an_origin_tooltip() {
    let mut app = app(GameMode::Creative);
    app.world_mut().resource_mut::<BlockPalette>().choices[0].identifier =
        "my_mod::cut_stone".into();
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Focus("block-3".into()));
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.detail_identifier, "my_mod::cut_stone");
    assert_eq!(view.detail_registry_name, "cut_stone");
    assert_eq!(view.detail_source, "von my_mod");
}

#[test]
fn custom_json_categories_drive_filtering_icons_and_selected_state_without_builtin_names() {
    let mut app = app(GameMode::Creative);
    {
        let mut palette = app.world_mut().resource_mut::<BlockPalette>();
        palette
            .item_categories
            .insert(ItemKind::Block(16), "example::workshop".into());
        palette
            .creative_categories
            .push(terra_player::interaction::CreativeCategory {
                id: "example::workshop".into(),
                name: "Workshop".into(),
                icon: ItemKind::Block(16),
                order: 150,
            });
    }
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        InventoryAction::Category("example::workshop".into()),
    );
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.browser_items.len(), 1);
    assert_eq!(view.browser_items[0].item_id, "block-16");
    let category = view
        .categories
        .iter()
        .find(|category| category.id == "example::workshop")
        .unwrap();
    assert!(category.selected);
    assert_eq!(category.preview_key, "Block preview 16");
    action(&mut app, InventoryAction::Category("unregistered".into()));
    assert_eq!(
        app.world().resource::<InventoryState>().category,
        "example::workshop"
    );
}

#[test]
fn switching_categories_preserves_an_authoritative_held_stack_and_never_queues_a_drop() {
    let mut app = app(GameMode::Creative);
    let held = terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 128,
    };
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(held);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Category("wood".into()));
    action(&mut app, InventoryAction::Category("all".into()));
    assert_eq!(
        app.world().resource::<ClientSession>().inventory.cursor,
        Some(held)
    );
    assert!(requests(&mut app).is_empty());
}

#[test]
fn survival_slots_keep_positions_and_request_move_without_mutating_inventory() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(2), 70);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Prop(ResourcePropKind::Flower), 3);
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.items.len(), terra_core::INVENTORY_SLOTS - 9);
    assert_eq!(view.hotbar[0].count, "70");
    assert_eq!(view.hotbar[1].count, "3");
    assert!(view.items.iter().all(|slot| slot.empty));
    action(&mut app, InventoryAction::Slot(1));
    action(&mut app, InventoryAction::Slot(12));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Move {
            from: 1,
            to: 12,
            count: 3
        }
    );
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slot(12)
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slot(1)
            .unwrap()
            .count,
        3
    );
}

#[test]
fn crafting_uses_real_slot_references_and_authoritative_request() {
    use terra_core::crafting::{CraftDestination, CraftGridCell};
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 2);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Slot(0));
    action(&mut app, InventoryAction::CraftCell(0));
    let view = app.world().resource::<InventoryView>();
    assert!(view.can_craft);
    assert_eq!(view.craft_result.item_id, "block-16");
    assert_eq!(view.craft_result.count, "4");
    action(&mut app, InventoryAction::Craft);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CraftReserved {
            cells: [
                Some(CraftGridCell { slot: 0, count: 1 }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None
            ],
            size: 2,
            station: None,
            count: 1,
            destination: CraftDestination::Inventory,
        }
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(6)),
        2
    );
}

#[test]
fn crafting_cell_stack_counts_determine_the_shift_click_batch_without_quantity_controls() {
    use terra_core::crafting::{CraftDestination, CraftGridCell};

    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(6),
        count: 3,
    });
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::CraftCell(0));
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Click {
            slot: 0,
            secondary: false
        }
    );
    let view = app.world().resource::<crafting::CraftingView>();
    assert_eq!(view.grid[0].count, "3");
    assert_eq!(view.maximum, "3");
    assert!(app.world().resource::<InventoryView>().hotbar[0].empty);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(6)),
        3
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    action(&mut app, pointer_action("craft", None, "block-16", false));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CraftReserved {
            cells: [
                Some(CraftGridCell { slot: 0, count: 3 }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None
            ],
            size: 2,
            station: None,
            count: 3,
            destination: CraftDestination::Inventory,
        }
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(6)),
        3
    );
}

#[test]
fn fully_reserved_backpack_sources_are_not_pickable_and_grid_pickup_takes_the_exact_stack() {
    for (secondary, taken, left) in [(false, 3, 0), (true, 2, 1)] {
        let mut app = app(GameMode::Survival);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(6),
            count: 3,
        });
        action(&mut app, InventoryAction::Toggle);
        action(&mut app, InventoryAction::CraftCell(0));
        apply_pointer_ack(&mut app);
        action(&mut app, pointer_action("hotbar", Some(0), "", false));
        assert!(requests(&mut app).is_empty());
        assert_eq!(
            app.world().resource::<crafting::CraftingView>().grid[0].count,
            "3"
        );
        action(
            &mut app,
            pointer_action("craft-cell", Some(0), "block-6", secondary),
        );
        assert_eq!(
            apply_pointer_ack(&mut app),
            ServerAction::TakeToCursor {
                slot: 0,
                count: taken
            }
        );
        let session = app.world().resource::<ClientSession>();
        assert_eq!(session.inventory.cursor.unwrap().count, taken);
        assert_eq!(session.inventory.count(ItemKind::Block(6)), left);
        let grid = &app.world().resource::<crafting::CraftingView>().grid;
        if left == 0 {
            assert!(grid[0].empty);
        } else {
            assert_eq!(grid[0].count, left.to_string());
        }
        assert!(app.world().resource::<InventoryView>().hotbar[0].empty);
        assert_eq!(
            session.inventory.count(ItemKind::Block(6)) + session.inventory.cursor.unwrap().count,
            3
        );
    }
}

#[test]
fn crafting_stack_reservation_survives_an_authoritative_revision_retry() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(6),
        count: 3,
    });
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::CraftCell(0));
    assert_eq!(requests(&mut app).len(), 1);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.revision += 1;
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::Conflict);
    }
    app.update();
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Click {
            slot: 0,
            secondary: false
        }
    );
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "3"
    );
    assert!(app.world().resource::<InventoryView>().hotbar[0].empty);
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
}

#[test]
fn partially_reserved_sources_allow_only_the_unreserved_count_to_reach_the_cursor() {
    for (secondary, taken) in [(false, 4), (true, 2)] {
        let mut app = app(GameMode::Survival);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .insert(ItemKind::Block(6), 5);
        action(&mut app, InventoryAction::Toggle);
        action(&mut app, pointer_action("catalog", None, "block-16", false));
        add_recipe(&mut app);
        assert_eq!(app.world().resource::<InventoryView>().hotbar[0].count, "4");
        action(
            &mut app,
            pointer_action("hotbar", Some(0), "block-6", secondary),
        );
        assert_eq!(
            apply_pointer_ack(&mut app),
            ServerAction::TakeToCursor {
                slot: 0,
                count: taken
            }
        );
        let session = app.world().resource::<ClientSession>();
        assert_eq!(session.inventory.cursor.unwrap().count, taken);
        assert_eq!(session.inventory.count(ItemKind::Block(6)), 5 - taken);
        assert_eq!(
            app.world().resource::<crafting::CraftingView>().grid[0].count,
            "1"
        );
    }
}

#[test]
fn shift_moving_a_partially_reserved_source_leaves_crafting_ingredients_in_place() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 5);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("catalog", None, "block-16", false));
    add_recipe(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-6", false),
    );
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Move {
            from: 0,
            to: 9,
            count: 4
        }
    );
    let session = app.world().resource::<ClientSession>();
    assert_eq!(session.inventory.slot(0).unwrap().count, 1);
    assert_eq!(session.inventory.slot(9).unwrap().count, 4);
    assert_eq!(session.inventory.count(ItemKind::Block(6)), 5);
    assert!(session.inventory.cursor.is_none());
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
    assert!(app.world().resource::<InventoryView>().hotbar[0].empty);
}

#[test]
fn taking_one_crafting_cell_preserves_other_cells_sharing_its_authoritative_source() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .choices
        .push(BlockChoice {
            id: 28,
            name: "Workbench".into(),
            identifier: ItemKind::Block(28).identifier(),
            description: "A workbench.".into(),
        });
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(16), 6);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("catalog", None, "block-28", false));
    add_recipe(&mut app);
    assert_eq!(
        app.world()
            .resource::<crafting::CraftingView>()
            .grid
            .iter()
            .map(|slot| slot.count.as_str())
            .collect::<Vec<_>>(),
        ["1", "1", "1", "1"]
    );
    action(
        &mut app,
        pointer_action("craft-cell", Some(1), "block-16", false),
    );
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::TakeToCursor { slot: 0, count: 1 }
    );
    let grid = &app.world().resource::<crafting::CraftingView>().grid;
    assert_eq!(grid[0].count, "1");
    assert!(grid[1].empty);
    assert_eq!(grid[2].count, "1");
    assert_eq!(grid[3].count, "1");
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.cursor.unwrap().count, 1);
    assert_eq!(inventory.count(ItemKind::Block(16)), 5);
}

#[test]
fn server_revision_clears_consumed_active_item_and_empty_hotbar_has_no_fallback() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Prop(ResourcePropKind::Flower), 1);
    action(&mut app, InventoryAction::Hotbar(0));
    assert_eq!(
        app.world().resource::<ActiveItem>().0,
        Some(ItemKind::Prop(ResourcePropKind::Flower))
    );
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.slots[0] = terra_core::ItemStack::EMPTY;
        session.inventory.revision += 1;
    }
    app.update();
    assert_eq!(app.world().resource::<ActiveItem>().0, None);
    assert!(app.world().resource::<BlockPalette>().selected().is_none());
    action(&mut app, InventoryAction::Hotbar(8));
    assert!(app.world().resource::<InventoryView>().selected_empty);
}

#[test]
fn rendered_previews_share_block_prop_and_avatar_handles() {
    use bevy::ui::widget::ImageNode;
    use tilt_ui::ImageMetadata;
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .init_resource::<BlockPreviewImages>()
        .init_resource::<PropPreviewImages>()
        .init_resource::<PlayerPreviewImage>()
        .add_systems(Update, sync_preview_images);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.world_mut()
        .resource_mut::<BlockPreviewImages>()
        .0
        .insert(3, handle.clone());
    app.world_mut()
        .resource_mut::<PropPreviewImages>()
        .0
        .insert(ResourcePropKind::Flower, handle.clone());
    app.world_mut().resource_mut::<PlayerPreviewImage>().0 = handle.clone();
    let entities: Vec<_> = [
        "Block preview 3",
        "Prop preview flower",
        "Player preview",
        "Menu background",
    ]
    .into_iter()
    .map(|alt| {
        app.world_mut()
            .spawn((
                ImageMetadata {
                    alt: Some(alt.into()),
                    ..default()
                },
                ImageNode::default(),
            ))
            .id()
    })
    .collect();
    app.update();
    for entity in &entities[..3] {
        let image = app.world().get::<ImageNode>(*entity).unwrap();
        assert_eq!(image.image, handle);
        assert_eq!(image.color, Color::WHITE);
    }
    assert_ne!(
        app.world().get::<ImageNode>(entities[3]).unwrap().image,
        handle
    );
    app.world_mut()
        .get_mut::<ImageNode>(entities[0])
        .unwrap()
        .color = Color::WHITE.with_alpha(0.0);
    app.update();
    assert_eq!(
        app.world()
            .get::<ImageNode>(entities[0])
            .unwrap()
            .color
            .alpha(),
        0.0,
        "The image bridge must not bypass TiltUI's invisible layout gate"
    );
}

#[test]
fn spectator_hides_the_hotbar_and_creative_restores_it_without_changing_inventory() {
    let mut app = app(GameMode::Creative);
    let inventory = app.world().resource::<ClientSession>().inventory.clone();
    for (mode, visible) in [
        (GameMode::Creative, true),
        (GameMode::Survival, true),
        (GameMode::Spectator, false),
        (GameMode::Creative, true),
    ] {
        app.world_mut().resource_mut::<ClientSession>().game_mode = mode;
        app.update();
        assert_eq!(app.world().resource::<InventoryView>().visible, visible);
        assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
    }
    {
        let mut menu = app.world_mut().resource_mut::<MenuState>();
        menu.session_active = true;
        menu.page = MenuPage::Pause;
    }
    app.update();
    assert!(app.world().resource::<InventoryView>().visible);
    app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Spectator;
    app.update();
    assert!(!app.world().resource::<InventoryView>().visible);
}

#[test]
fn held_close_click_does_not_mine_and_session_leave_does_not_recapture() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(&mut app, InventoryAction::Close);
    app.update();
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(!app.world().resource::<PlayerInputBlock>().0);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<Messages<CaptureRequest>>()
        .clear();
    app.world_mut().resource_mut::<MenuState>().page = MenuPage::Loading;
    app.update();
    assert!(!app.world().resource::<InventoryView>().visible);
    assert!(!app.world().resource::<InventoryState>().blocks_input());
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .next()
            .is_none()
    );
}

#[test]
fn search_focus_does_not_trigger_inventory_binding_and_escape_clears_focus() {
    let mut app = app(GameMode::Creative);
    app.world_mut()
        .resource_mut::<ControlsConfig>()
        .rebind(GameAction::Inventory, InputBinding::Key(KeyCode::KeyI))
        .unwrap();
    action(&mut app, InventoryAction::Toggle);
    let search = app
        .world_mut()
        .spawn(ElementId("inventory-search".into()))
        .id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(search, bevy::input_focus::FocusCause::Navigated);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyI);
    app.update();
    assert!(app.world().resource::<InventoryView>().open);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<InventoryView>().open);
    assert!(app.world().resource::<InputFocus>().get().is_none());
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Playing);
}

#[test]
fn late_rail_owned_wheel_never_changes_slots_or_replays_and_normal_wheel_applies_immediately() {
    let mut app = app(GameMode::Survival);
    let track = ItemKind::for_wood(196, terra_core::WoodSpecies::Birch);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(
            0,
            terra_core::ItemStack {
                item: track,
                count: 8,
            },
        );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(
            1,
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 8,
            },
        );
    action(&mut app, InventoryAction::Hotbar(0));
    app.insert_resource(terra_player::interaction::RailBuildContext {
        captures_wheel: true,
        active: true,
        item: Some(track),
        ..default()
    });
    let wheel = || MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    };
    app.world_mut().write_message(wheel());
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().hotbar, 0);
    assert_eq!(app.world().resource::<ActiveItem>().0, Some(track));
    assert_eq!(app.world().resource::<SelectedInventorySlot>().0, Some(0));
    app.world_mut()
        .resource_mut::<terra_player::interaction::RailBuildContext>()
        .captures_wheel = false;
    app.update();
    assert_eq!(
        app.world().resource::<InventoryState>().hotbar,
        0,
        "captured wheel must be drained"
    );
    app.world_mut()
        .resource_mut::<Messages<InventoryAction>>()
        .clear();
    app.world_mut().write_message(wheel());
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().hotbar, 1);
    assert_eq!(
        app.world().resource::<ActiveItem>().0,
        Some(ItemKind::Block(3))
    );
    assert_eq!(app.world().resource::<SelectedInventorySlot>().0, Some(1));
    assert_eq!(app.world().resource::<BlockPalette>().selected, 0);
    assert!(
        app.world()
            .resource::<terra_player::interaction::RailBuildContext>()
            .hotbar_changed
    );
    assert!(
        app.world()
            .resource::<Messages<InventoryAction>>()
            .is_empty(),
        "late navigation applies shared helper, without a next-frame event"
    );
}

#[test]
fn wheel_selects_real_hotbar_slots_and_leaves_open_inventory_scrolling_alone() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(2), 5);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Prop(ResourcePropKind::Fern), 2);
    app.update();
    let wheel = || MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    };
    app.world_mut().write_message(wheel());
    app.update();
    assert_eq!(
        app.world().resource::<ActiveItem>().0,
        Some(ItemKind::Prop(ResourcePropKind::Fern))
    );
    assert_eq!(app.world().resource::<SelectedInventorySlot>().0, Some(1));
    app.world_mut().write_message(wheel());
    app.update();
    assert!(app.world().resource::<ActiveItem>().0.is_none());
    action(&mut app, InventoryAction::Toggle);
    app.world_mut().write_message(wheel());
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().hotbar, 2);
}

fn pointer_action(
    command: &str,
    index: Option<u8>,
    item: &str,
    secondary: bool,
) -> InventoryAction {
    InventoryAction::Pointer {
        command: command.into(),
        index,
        item: item.into(),
        secondary,
    }
}

#[test]
fn real_browser_pointer_grants_to_inventory_without_replacing_the_selected_slot() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("catalog", None, "block-3", false));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeQuickMove {
            item: ItemKind::Block(3),
            count: 1
        }
    );
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
    assert!(app.world().resource::<ActiveItem>().0.is_none());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    action(
        &mut app,
        pointer_action("catalog", None, "prop-flower", true),
    );
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeQuickMove {
            item: ItemKind::Prop(ResourcePropKind::Flower),
            count: 1
        }
    );
}

#[test]
fn mouse_requests_wait_for_an_authoritative_response_before_using_the_next_revision() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 40);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("hotbar", Some(0), "block-3", true));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Click {
            slot: 0,
            secondary: true
        }
    );
    action(&mut app, pointer_action("slot", Some(12), "", false));
    assert!(requests(&mut app).is_empty());
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.inventory.revision = 19;
        session.inventory.cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 20,
        });
    }
    app.update();
    let next = requests(&mut app);
    assert_eq!(next[0].revision, 19);
    assert_eq!(
        next[0].action,
        ServerAction::Click {
            slot: 12,
            secondary: false
        }
    );
}

#[test]
fn held_drag_collects_distinct_destinations_and_releases_one_atomic_request() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 17,
    });
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(&mut app, pointer_action("slot", Some(10), "", false));
    for slot in [11, 10, 12] {
        action(
            &mut app,
            InventoryAction::Hover {
                command: "slot".into(),
                index: Some(slot),
                item: String::new(),
            },
        );
    }
    assert!(requests(&mut app).is_empty());
    for index in [10, 11, 12] {
        let slot = app
            .world()
            .resource::<InventoryView>()
            .items
            .iter()
            .find(|slot| slot.index == index.to_string())
            .unwrap();
        assert!(!slot.empty);
        assert_eq!(slot.count, "5");
    }
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    let mut slots = [None; terra_core::INVENTORY_SLOTS];
    slots[..3].copy_from_slice(&[Some(10), Some(11), Some(12)]);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Drag {
            slots,
            secondary: false
        }
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .unwrap()
            .count,
        17
    );
}

#[test]
fn cursor_drop_and_shift_quick_move_use_the_same_authoritative_queue() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 7,
    });
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Outside(true));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::DropCursor { all: false }
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-3", false),
    );
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::QuickMove { slot: 0 }
    );
}

#[test]
fn prop_descriptions_come_from_the_loaded_registry_metadata() {
    let mut app = app(GameMode::Creative);
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .prop_descriptions
        .insert(ResourcePropKind::Grass, "Editable JSON description".into());
    action(&mut app, InventoryAction::Toggle);
    assert_eq!(
        app.world()
            .resource::<InventoryView>()
            .browser_items
            .iter()
            .find(|item| item.item_id == "prop-grass_tuft")
            .unwrap()
            .description,
        "Editable JSON description"
    );
}

#[test]
fn middle_pick_selects_an_existing_hotbar_block_or_requests_one_block_only_in_creative() {
    let mut app = app(GameMode::Creative);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session
            .inventory
            .slots
            .resize(9, terra_core::ItemStack::EMPTY);
        session.inventory.slots[4] = terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 12,
        };
    }
    action(&mut app, InventoryAction::PickBlock(3));
    assert!(requests(&mut app).is_empty());
    assert_eq!(app.world().resource::<SelectedInventorySlot>().0, Some(4));
    assert_eq!(
        app.world().resource::<ActiveItem>().0,
        Some(ItemKind::Block(3))
    );
    action(&mut app, InventoryAction::PickBlock(2));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeSet {
            slot: 4,
            item: ItemKind::Block(2),
            count: 1
        }
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slot(4)
            .unwrap()
            .item,
        ItemKind::Block(3)
    );
    app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Survival;
    action(&mut app, InventoryAction::PickBlock(4));
    assert!(requests(&mut app).is_empty());
}

#[test]
fn actual_middle_mouse_uses_the_hit_block_at_negative_coordinates() {
    use terra_voxel::{BlockSection, MaterialId, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};
    let mut app = app(GameMode::Creative);
    let position = SectionPos::new(-1, 0, -1);
    let mut data = BlockSection::empty(position).unwrap();
    data.set_block([15, 0, 15], MaterialId::Stone as u16)
        .unwrap();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        position,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.insert_resource(chunks)
        .insert_resource(terra_player::TargetWorld(Some(
            terra_player::WorldTarget::Block(BlockHit {
                position: [-1, 0, -1],
                normal: [0, 1, 0],
                distance: 2.0,
            }),
        )));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Middle);
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeSet {
            slot: 0,
            item: ItemKind::Block(MaterialId::Stone as u16),
            count: 1
        }
    );
}

#[test]
fn q_drops_one_and_either_shift_drops_the_whole_selected_stack() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 3);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyQ);
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::DropSlot {
            slot: 0,
            all: false
        }
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        keys.press(KeyCode::ShiftRight);
        keys.press(KeyCode::KeyQ);
    }
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::DropSlot { slot: 0, all: true }
    );
}

#[test]
fn a_revision_conflict_retries_the_unapplied_action_before_the_next_mouse_request() {
    use terra_network::protocol::inventory::InventoryOutcome;
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 128);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("hotbar", Some(0), "block-3", true));
    let original = requests(&mut app)[0].action;
    action(&mut app, pointer_action("hotbar", Some(0), "", false));
    assert!(requests(&mut app).is_empty());
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.inventory.revision = 3;
        session.last_inventory = Some(InventoryOutcome::Conflict);
    }
    app.update();
    let retry = requests(&mut app);
    assert_eq!(retry[0].action, original);
    assert_eq!(retry[0].revision, 3);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.inventory.revision = 4;
        session.last_inventory = Some(InventoryOutcome::Applied);
        session.inventory.slots[0].count = 64;
        session.inventory.cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 64,
        });
    }
    app.update();
    let next = requests(&mut app);
    assert_eq!(
        next[0].action,
        ServerAction::Click {
            slot: 0,
            secondary: false
        }
    );
    assert_eq!(next[0].revision, 4);
}

#[test]
fn closing_before_a_pickup_ack_returns_the_eventual_cursor_stack_without_losing_it() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 128);
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-3", false),
    );
    assert_eq!(requests(&mut app).len(), 1);
    action(&mut app, InventoryAction::Close);
    assert!(requests(&mut app).is_empty());
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.inventory.slots[0] = terra_core::ItemStack::EMPTY;
        session.inventory.cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 128,
        });
        session.inventory.revision += 1;
    }
    app.update();
    assert_eq!(requests(&mut app)[0].action, ServerAction::ReturnCursor);
    assert!(!app.world().resource::<InventoryState>().open);
}

#[test]
fn shifted_catalog_selection_lets_the_server_find_space_instead_of_overwriting_a_stale_slot() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    action(&mut app, pointer_action("catalog", None, "block-3", false));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeQuickMove {
            item: ItemKind::Block(3),
            count: 128
        }
    );
    action(&mut app, pointer_action("catalog", None, "block-2", false));
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeQuickMove {
            item: ItemKind::Block(2),
            count: 128
        }
    );
}

#[test]
fn a_fast_double_click_collects_after_the_first_pickup_ack() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 20);
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-3", false),
    );
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Click {
            slot: 0,
            secondary: false
        }
    );
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-3", false),
    );
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Collect { slot: 0 }
    );
}

#[test]
fn plain_wheel_over_slots_or_crafting_keeps_items_unchanged_for_scrolling() {
    for command in ["slot", "craft-cell"] {
        for carried in [false, true] {
            let mut app = app(GameMode::Survival);
            {
                let mut session = app.world_mut().resource_mut::<ClientSession>();
                session.inventory.insert(ItemKind::Block(3), 4);
                if carried {
                    session.inventory.cursor = Some(terra_core::ItemStack {
                        item: ItemKind::Block(3),
                        count: 2,
                    });
                }
            }
            action(&mut app, InventoryAction::Toggle);
            action(
                &mut app,
                InventoryAction::Hover {
                    command: command.into(),
                    index: Some(0),
                    item: "block-3".into(),
                },
            );
            let before = app.world().resource::<ClientSession>().inventory.clone();
            app.world_mut().write_message(MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: 2.0,
                window: Entity::PLACEHOLDER,
                phase: bevy::input::touch::TouchPhase::Moved,
            });
            app.update();
            assert!(requests(&mut app).is_empty(), "{command}, cursor={carried}");
            assert_eq!(app.world().resource::<ClientSession>().inventory, before);
            assert_eq!(app.world().resource::<InventoryState>().hotbar, 0);
        }
    }
}

fn chest_snapshot(anchor: [i64; 3], double: bool) -> ContainerSnapshot {
    let half = |position, revision| {
        let mut inventory = terra_core::Inventory::default();
        inventory.insert(ItemKind::Block(3), 4);
        inventory.revision = revision;
        ContainerHalf {
            position,
            inventory,
        }
    };
    let mut halves = vec![half(anchor, 7)];
    if double {
        halves.push(half([anchor[0] + 1, anchor[1], anchor[2]], 11));
    }
    ContainerSnapshot {
        anchor,
        halves,
        workbench: None,
        view: ContainerView::Primary,
        attached_campfire: false,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        furnace: None,
        crusher: None,
        mixer: None,
        filter: None,
        distributor: None,
    }
}

fn queue_app(container: Option<ContainerSnapshot>) -> App {
    let mut app = App::new();
    app.init_resource::<ClientSession>()
        .init_resource::<crafting::CraftingState>()
        .init_resource::<pointer::InventoryQueue>()
        .add_message::<InventoryRequest>()
        .add_message::<ContainerRequest>()
        .add_systems(Update, pointer::send_requests);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.ready = true;
        session.inventory.insert(ItemKind::Block(3), 4);
        session.inventory.revision = 9;
        session.container = container;
    }
    app
}

fn container_requests(app: &mut App) -> Vec<ContainerCommand> {
    app.world_mut()
        .resource_mut::<Messages<ContainerRequest>>()
        .drain()
        .map(|request| request.0)
        .collect()
}

#[test]
fn queue_routes_world_slots_to_the_container_and_non_container_actions_to_inventory() {
    let anchor = [-6, 70, 9];
    let mut backpack = queue_app(None);
    backpack
        .world_mut()
        .resource_mut::<pointer::InventoryQueue>()
        .push(ServerAction::Click {
            slot: 0,
            secondary: false,
        });
    backpack.update();
    assert!(container_requests(&mut backpack).is_empty());
    assert_eq!(
        requests(&mut backpack)[0].action,
        ServerAction::Click {
            slot: 0,
            secondary: false,
        }
    );
    let mut app = queue_app(Some(chest_snapshot(anchor, true)));
    {
        let mut queue = app.world_mut().resource_mut::<pointer::InventoryQueue>();
        queue.owner = Some(anchor);
        queue.push(ServerAction::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: true,
        });
        queue.push(ServerAction::CreativeQuickMove {
            item: ItemKind::Block(3),
            count: 1,
        });
    }
    app.update();
    assert!(requests(&mut app).is_empty());
    let commands = container_requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].anchor, anchor);
    assert_eq!(commands[0].inventory_revision, 9);
    assert_eq!(commands[0].revisions, [7, 11]);
    assert_eq!(
        commands[0].action,
        terra_core::containers::ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: true
        }
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    app.update();
    assert!(container_requests(&mut app).is_empty());
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeQuickMove {
            item: ItemKind::Block(3),
            count: 1,
        }
    );
}

#[test]
fn queue_discards_a_transfer_from_a_closed_or_replaced_container_context() {
    let old = [-6, 70, 9];
    for replacement in [None, Some(chest_snapshot([10, 71, 20], false))] {
        let mut app = queue_app(Some(chest_snapshot(old, false)));
        {
            let mut queue = app.world_mut().resource_mut::<pointer::InventoryQueue>();
            queue.owner = Some(old);
            queue.push(ServerAction::QuickMove { slot: 0 });
        }
        app.world_mut().resource_mut::<ClientSession>().container = replacement;
        app.update();
        assert!(requests(&mut app).is_empty());
        assert!(container_requests(&mut app).is_empty());
        assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    }
    let mut app = queue_app(None);
    app.world_mut()
        .resource_mut::<pointer::InventoryQueue>()
        .push(ServerAction::QuickMove { slot: 0 });
    app.world_mut().resource_mut::<ClientSession>().container = Some(chest_snapshot(old, false));
    app.update();
    assert!(requests(&mut app).is_empty());
    assert!(container_requests(&mut app).is_empty());
}

#[test]
fn double_chest_drag_preserves_every_virtual_slot_index() {
    let anchor = [-6, 70, 9];
    let mut app = queue_app(Some(chest_snapshot(anchor, true)));
    {
        let mut queue = app.world_mut().resource_mut::<pointer::InventoryQueue>();
        queue.owner = Some(anchor);
        queue.push(ServerAction::ViewDrag {
            slots: std::array::from_fn(|index| Some(index as u8)),
            extra: std::array::from_fn(|index| Some((index + 32) as u8)),
            secondary: false,
        });
    }
    app.update();
    assert!(requests(&mut app).is_empty());
    let commands = container_requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].action,
        terra_core::containers::ContainerGesture::Drag {
            slots: (0..terra_core::containers::CONTAINER_VIEW_SLOTS as u8).collect(),
            secondary: false,
        }
    );
}

#[test]
fn an_idle_open_chest_does_not_mark_inventory_or_crafting_state_changed_every_frame() {
    #[derive(Resource, Default)]
    struct IdleFlags(Vec<(bool, bool)>);
    let mut app = app(GameMode::Survival);
    app.init_resource::<IdleFlags>().add_systems(
        PostUpdate,
        |inventory: Res<InventoryState>,
         crafting: Res<crafting::CraftingState>,
         mut flags: ResMut<IdleFlags>| {
            flags
                .0
                .push((inventory.is_changed(), crafting.is_changed()));
        },
    );
    action(&mut app, InventoryAction::Toggle);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([-6, 70, 9], false));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<IdleFlags>().0.last(),
        Some(&(false, false))
    );
}

#[test]
fn denied_chest_open_closes_the_pending_ui_and_returns_a_held_cursor() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 4,
    });
    action(&mut app, InventoryAction::OpenChest([-6, 70, 9]));
    assert!(matches!(
        requests(&mut app)[0].action,
        ServerAction::OpenContainer { .. }
    ));
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::ContainerUnavailable);
    }
    app.update();
    app.update();
    assert!(!app.world().resource::<InventoryState>().open);
    assert!(
        app.world()
            .resource::<InventoryState>()
            .container_open_response
            .is_none()
    );
    assert_eq!(requests(&mut app)[0].action, ServerAction::ReturnCursor);
    assert!(!app.world().resource::<PlayerInputBlock>().0);
}

#[test]
fn an_unrelated_ack_before_open_dispatch_does_not_fail_the_pending_chest() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(3), 4);
    app.world_mut()
        .resource_mut::<pointer::InventoryQueue>()
        .push(ServerAction::QuickMove { slot: 0 });
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::QuickMove { slot: 0 }
    );
    action(&mut app, InventoryAction::OpenChest([-6, 70, 9]));
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::Applied);
    }
    app.update();
    assert!(app.world().resource::<InventoryState>().open);
    assert!(matches!(
        requests(&mut app)[0].action,
        ServerAction::OpenContainer { .. }
    ));
    app.update();
    assert!(app.world().resource::<InventoryState>().open);
    assert!(
        app.world()
            .resource::<InventoryState>()
            .container_open_response
            .is_some()
    );
}

#[test]
fn closing_before_the_open_ack_preserves_cursor_return_and_closes_the_late_snapshot() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 4,
    });
    action(&mut app, InventoryAction::OpenChest([-6, 70, 9]));
    requests(&mut app);
    action(&mut app, InventoryAction::Close);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.container = Some(chest_snapshot([-6, 70, 9], false));
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::Applied);
    }
    app.update();
    assert!(!app.world().resource::<InventoryState>().open);
    assert_eq!(requests(&mut app)[0].action, ServerAction::ReturnCursor);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        assert!(session.inventory.return_cursor());
        session.inventory_response += 1;
    }
    app.update();
    assert_eq!(requests(&mut app)[0].action, ServerAction::CloseContainer);
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert!(inventory.cursor.is_none());
    assert_eq!(inventory.count(ItemKind::Block(3)), 4);
}

#[test]
fn ordinary_player_inventory_closes_and_reopens_without_a_container_ack() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Close);
    assert!(requests(&mut app).is_empty());
    assert!(
        !app.world()
            .resource::<pointer::InventoryQueue>()
            .has_container_lifecycle()
    );
    action(&mut app, InventoryAction::Toggle);
    assert!(app.world().resource::<InventoryState>().open);
    assert!(!app.world().resource::<InventoryState>().container_active);
}

#[test]
fn rapid_reopen_waits_for_old_container_lifecycle_without_adopting_its_late_snapshot() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::OpenChest([-6, 70, 9]));
    assert!(matches!(
        requests(&mut app)[0].action,
        ServerAction::OpenContainer { .. }
    ));
    action(&mut app, InventoryAction::Close);
    for fresh in [
        InventoryAction::Toggle,
        InventoryAction::OpenChest([12, 70, 9]),
        InventoryAction::OpenWorkbench([14, 70, 9]),
    ] {
        action(&mut app, fresh);
        let state = app.world().resource::<InventoryState>();
        assert!(!state.open);
        assert!(!state.container_active);
        assert!(state.container_open_response.is_none());
    }
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.container = Some(chest_snapshot([-6, 70, 9], false));
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::Applied);
    }
    app.update();
    assert!(!app.world().resource::<InventoryState>().open);
    assert!(!app.world().resource::<InventoryState>().container_active);
    assert_eq!(requests(&mut app)[0].action, ServerAction::CloseContainer);
    action(&mut app, InventoryAction::Toggle);
    assert!(!app.world().resource::<InventoryState>().open);
    assert!(requests(&mut app).is_empty());
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.container = None;
        session.inventory_response += 1;
    }
    app.update();
    action(&mut app, InventoryAction::Toggle);
    assert!(app.world().resource::<InventoryState>().open);
    assert!(!app.world().resource::<InventoryView>().container);
}

#[test]
fn server_forced_chest_close_returns_cursor_through_the_player_inventory_channel() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([-6, 70, 9], false));
    app.update();
    assert!(app.world().resource::<InventoryState>().container_active);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 4,
        });
        session.container = None;
    }
    app.update();
    assert!(!app.world().resource::<InventoryState>().open);
    assert_eq!(requests(&mut app)[0].action, ServerAction::ReturnCursor);
    assert!(container_requests(&mut app).is_empty());
}

#[test]
fn wheel_over_an_open_inventory_slot_serializes_single_item_transfers() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        InventoryAction::Hover {
            command: "slot".into(),
            index: Some(12),
            item: "block-3".into(),
        },
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    app.world_mut().write_message(MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: 2.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Wheel {
            slot: 12,
            to_cursor: true
        }
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory_response += 1;
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::Wheel {
            slot: 12,
            to_cursor: true
        }
    );
    assert_eq!(app.world().resource::<InventoryState>().hotbar, 0);
}

fn apply_pointer_ack(app: &mut App) -> ServerAction {
    let commands = requests(app);
    assert_eq!(commands.len(), 1);
    apply_pointer_command(app, commands[0])
}

fn apply_pointer_command(app: &mut App, command: InventoryCommand) -> ServerAction {
    let recipes = app
        .world()
        .resource::<terra_core::crafting::RecipeBook>()
        .clone();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        assert_eq!(command.revision, session.inventory.revision);
        let applied = match command.action {
            ServerAction::Click { slot, secondary } => {
                session.inventory.click(usize::from(slot), secondary)
            }
            ServerAction::TakeToCursor { slot, count } => {
                session.inventory.take_cursor(usize::from(slot), count)
            }
            ServerAction::TransferCursor {
                item,
                counts,
                to_cursor,
            } => session.inventory.transfer_cursor(item, counts, to_cursor),
            ServerAction::Move { from, to, count } => {
                session
                    .inventory
                    .move_slot(usize::from(from), usize::from(to), count)
            }
            ServerAction::CollectFrom { slot } => session.inventory.collect_from(usize::from(slot)),
            ServerAction::QuickMove { slot } => {
                let count = session
                    .inventory
                    .slot(usize::from(slot))
                    .map_or(0, |stack| stack.count);
                session.inventory.quick_move(usize::from(slot), count)
            }
            ServerAction::Wheel { slot, to_cursor } => {
                session.inventory.wheel(usize::from(slot), to_cursor)
            }
            ServerAction::ProtectedMove {
                slot,
                kind,
                reserved,
            } => session.inventory.with_reserved(reserved, |inventory| {
                use terra_network::protocol::inventory::ProtectedMoveKind;
                match kind {
                    ProtectedMoveKind::QuickMove => {
                        inventory.quick_move(usize::from(slot), STACK_LIMIT)
                    }
                    ProtectedMoveKind::Collect => inventory.collect(usize::from(slot)),
                    ProtectedMoveKind::CollectFrom => inventory.collect_from(usize::from(slot)),
                    ProtectedMoveKind::WheelToCursor => inventory.wheel(usize::from(slot), true),
                }
            }),
            ServerAction::CraftReserved {
                cells,
                size,
                station,
                count,
                destination,
            } => recipes.craft_reserved_grid(
                &mut session.inventory,
                &cells[..usize::from(size * size)],
                usize::from(size),
                if station.is_some() {
                    terra_core::crafting::CraftStation::Workbench
                } else {
                    terra_core::crafting::CraftStation::Hand
                },
                count,
                destination,
            ),
            _ => panic!("Unexpected pointer action in acknowledgement fixture"),
        };
        assert!(applied);
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::Applied);
    }
    app.update();
    command.action
}

fn hover_slot(app: &mut App, slot: u8, item: &str) {
    action(
        app,
        InventoryAction::Hover {
            command: if slot < 9 { "hotbar" } else { "slot" }.into(),
            index: Some(slot),
            item: item.into(),
        },
    );
}

fn partially_reserved_wood_app() -> App {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .slots = vec![
        terra_core::ItemStack {
            item: ItemKind::Block(6),
            count: 5,
        },
        terra_core::ItemStack {
            item: ItemKind::Block(6),
            count: 4,
        },
        terra_core::ItemStack {
            item: ItemKind::Block(2),
            count: 7,
        },
    ];
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("catalog", None, "block-16", false));
    add_recipe(&mut app);
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
    app
}

fn reserved_wood_counts() -> [u16; terra_core::INVENTORY_SLOTS] {
    let mut reserved = [0; terra_core::INVENTORY_SLOTS];
    reserved[0] = 1;
    reserved
}

#[test]
fn double_click_collect_leaves_reserved_crafting_ingredients_in_other_slots() {
    use terra_network::protocol::inventory::ProtectedMoveKind;
    let mut app = partially_reserved_wood_app();
    action(
        &mut app,
        pointer_action("hotbar", Some(1), "block-6", false),
    );
    apply_pointer_ack(&mut app);
    action(
        &mut app,
        pointer_action("hotbar", Some(1), "block-6", false),
    );
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::ProtectedMove {
            slot: 1,
            kind: ProtectedMoveKind::Collect,
            reserved: reserved_wood_counts(),
        }
    );
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.slot(0).unwrap().count, 1);
    assert_eq!(inventory.cursor.unwrap().count, 8);
    assert_eq!(
        inventory.count(ItemKind::Block(6)) + inventory.cursor.unwrap().count,
        9
    );
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
}

#[test]
fn collect_sweeps_transfer_only_the_unreserved_portion_of_a_crafting_source() {
    use terra_network::protocol::inventory::ProtectedMoveKind;
    let mut app = partially_reserved_wood_app();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(
        &mut app,
        pointer_action("hotbar", Some(1), "block-6", false),
    );
    apply_pointer_ack(&mut app);
    hover_slot(&mut app, 0, "block-6");
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::ProtectedMove {
            slot: 0,
            kind: ProtectedMoveKind::CollectFrom,
            reserved: reserved_wood_counts(),
        }
    );
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.slot(0).unwrap().count, 1);
    assert_eq!(inventory.cursor.unwrap().count, 8);
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
}

#[test]
fn shift_sweeps_keep_reserved_crafting_items_in_their_source_slot() {
    use terra_network::protocol::inventory::ProtectedMoveKind;
    let mut app = partially_reserved_wood_app();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    action(
        &mut app,
        pointer_action("hotbar", Some(2), "block-2", false),
    );
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::ProtectedMove {
            slot: 2,
            kind: ProtectedMoveKind::QuickMove,
            reserved: reserved_wood_counts(),
        }
    );
    hover_slot(&mut app, 0, "block-6");
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::ProtectedMove {
            slot: 0,
            kind: ProtectedMoveKind::QuickMove,
            reserved: reserved_wood_counts(),
        }
    );
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.slot(0).unwrap().count, 1);
    assert_eq!(inventory.slot(9).unwrap().item, ItemKind::Block(2));
    assert_eq!(inventory.slot(10).unwrap().count, 4);
    assert_eq!(inventory.count(ItemKind::Block(6)), 9);
    assert!(inventory.cursor.is_none());
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
}

#[test]
fn wheel_batches_carry_reservations_through_each_authoritative_transfer() {
    use terra_network::protocol::inventory::ProtectedMoveKind;
    let mut app = partially_reserved_wood_app();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(6),
        count: 1,
    });
    hover_slot(&mut app, 0, "block-6");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    app.world_mut().write_message(MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: 2.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    for _ in 0..2 {
        assert_eq!(
            apply_pointer_ack(&mut app),
            ServerAction::ProtectedMove {
                slot: 0,
                kind: ProtectedMoveKind::WheelToCursor,
                reserved: reserved_wood_counts(),
            }
        );
    }
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.slot(0).unwrap().count, 3);
    assert_eq!(inventory.cursor.unwrap().count, 3);
    assert_eq!(
        inventory.count(ItemKind::Block(6)) + inventory.cursor.unwrap().count,
        10
    );
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
}

#[test]
fn q_and_shift_q_do_not_drop_fully_reserved_crafting_ingredients() {
    let mut app = partially_reserved_wood_app();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.slots[0].count = 1;
        session.inventory.revision += 1;
    }
    app.update();
    for shift in [false, true] {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            if shift {
                keys.press(KeyCode::ShiftLeft);
            }
            keys.press(KeyCode::KeyQ);
        }
        app.update();
        assert!(requests(&mut app).is_empty());
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .unwrap()
                .count,
            1
        );
        assert_eq!(
            app.world().resource::<crafting::CraftingView>().grid[0].count,
            "1"
        );
    }
}

#[test]
fn shift_q_drops_only_unreserved_items_and_cursor_guard_rejects_a_failed_take() {
    use terra_network::protocol::inventory::InventoryOutcome;
    for succeed in [true, false] {
        let mut app = partially_reserved_wood_app();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyQ);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        if succeed {
            assert_eq!(
                apply_pointer_ack(&mut app),
                ServerAction::TakeToCursor { slot: 0, count: 4 }
            );
            let commands = requests(&mut app);
            assert_eq!(commands.len(), 1);
            assert_eq!(commands[0].action, ServerAction::DropCursor { all: true });
            let dropped = {
                let mut session = app.world_mut().resource_mut::<ClientSession>();
                let dropped = session.inventory.take_stack(None, true).unwrap();
                session.inventory_response += 1;
                session.last_inventory = Some(InventoryOutcome::Applied);
                dropped
            };
            app.update();
            assert_eq!(dropped.item, ItemKind::Block(6));
            assert_eq!(dropped.count, 4);
            let inventory = &app.world().resource::<ClientSession>().inventory;
            assert_eq!(inventory.slot(0).unwrap().count, 1);
            assert!(inventory.cursor.is_none());
            assert_eq!(inventory.count(ItemKind::Block(6)) + dropped.count, 9);
        } else {
            let commands = requests(&mut app);
            assert_eq!(commands.len(), 1);
            assert_eq!(
                commands[0].action,
                ServerAction::TakeToCursor { slot: 0, count: 4 }
            );
            {
                let mut session = app.world_mut().resource_mut::<ClientSession>();
                session.inventory_response += 1;
                session.last_inventory = Some(InventoryOutcome::Invalid);
            }
            app.update();
            assert!(requests(&mut app).is_empty());
            assert_eq!(
                app.world()
                    .resource::<ClientSession>()
                    .inventory
                    .count(ItemKind::Block(6)),
                9
            );
        }
        assert_eq!(
            app.world().resource::<crafting::CraftingView>().grid[0].count,
            "1"
        );
    }
}

#[test]
fn legacy_hotbar_moves_transfer_only_unreserved_counts_and_cannot_replace_reserved_targets() {
    let mut app = partially_reserved_wood_app();
    action(&mut app, InventoryAction::Slot(0));
    action(&mut app, InventoryAction::Hotbar(3));
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Move {
            from: 0,
            to: 3,
            count: 4
        }
    );
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.slot(0).unwrap().count, 1);
    assert_eq!(inventory.slot(3).unwrap().count, 4);
    assert_eq!(inventory.count(ItemKind::Block(6)), 9);
    assert_eq!(
        app.world().resource::<crafting::CraftingView>().grid[0].count,
        "1"
    );
    action(&mut app, InventoryAction::Slot(2));
    action(&mut app, InventoryAction::Hotbar(0));
    assert!(requests(&mut app).is_empty());
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slot(0)
            .unwrap()
            .item,
        ItemKind::Block(6)
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .slot(2)
            .unwrap()
            .count,
        7
    );
}

#[test]
fn right_sweep_places_one_on_each_entry_including_a_return_to_an_earlier_slot() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 17,
    });
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    action(&mut app, pointer_action("slot", Some(10), "", true));
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Click {
            slot: 10,
            secondary: true
        }
    );
    hover_slot(&mut app, 11, "");
    apply_pointer_ack(&mut app);
    action(
        &mut app,
        InventoryAction::Leave {
            command: "slot".into(),
            index: Some(11),
            item: String::new(),
        },
    );
    hover_slot(&mut app, 10, "block-3");
    apply_pointer_ack(&mut app);
    hover_slot(&mut app, 12, "block-2");
    assert!(requests(&mut app).is_empty());
    hover_slot(&mut app, 10, "block-3");
    apply_pointer_ack(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);
    app.update();
    assert!(requests(&mut app).is_empty());
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.slot(10).unwrap().count, 3);
    assert_eq!(inventory.slot(11).unwrap().count, 1);
    assert_eq!(inventory.cursor.unwrap().count, 13);
    assert_eq!(
        inventory.count(ItemKind::Block(3)) + inventory.cursor.unwrap().count,
        17
    );
}

#[test]
fn left_pickup_sweep_collects_only_visited_matching_stacks_and_conserves_the_remainder() {
    let mut app = app(GameMode::Survival);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.slots = vec![
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 40,
            },
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 100,
            },
            terra_core::ItemStack {
                item: ItemKind::Block(2),
                count: 7,
            },
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 3,
            },
        ];
    }
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-3", false),
    );
    apply_pointer_ack(&mut app);
    hover_slot(&mut app, 1, "block-3");
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::CollectFrom { slot: 1 }
    );
    hover_slot(&mut app, 2, "block-2");
    assert!(requests(&mut app).is_empty());
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert_eq!(inventory.cursor.unwrap().count, 128);
    assert_eq!(inventory.slot(1).unwrap().count, 12);
    assert_eq!(inventory.slot(2).unwrap().count, 7);
    assert_eq!(inventory.slot(3).unwrap().count, 3);
    assert_eq!(
        inventory.count(ItemKind::Block(3)) + inventory.cursor.unwrap().count,
        143
    );
}

#[test]
fn shift_left_sweep_quick_moves_each_visited_stack_once() {
    let mut app = app(GameMode::Survival);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.slots = vec![
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 12,
            },
            terra_core::ItemStack {
                item: ItemKind::Block(2),
                count: 5,
            },
            terra_core::ItemStack {
                item: ItemKind::Block(4),
                count: 3,
            },
        ];
    }
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    action(
        &mut app,
        pointer_action("hotbar", Some(0), "block-3", false),
    );
    apply_pointer_ack(&mut app);
    for (slot, item) in [(1, "block-2"), (2, "block-4")] {
        hover_slot(&mut app, slot, item);
        assert_eq!(
            apply_pointer_ack(&mut app),
            ServerAction::QuickMove { slot }
        );
    }
    hover_slot(&mut app, 0, "block-3");
    assert!(requests(&mut app).is_empty());
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert!((0..3).all(|slot| inventory.slot(slot).is_none()));
    assert_eq!(inventory.slot(9).unwrap().count, 12);
    assert_eq!(inventory.slot(10).unwrap().count, 5);
    assert_eq!(inventory.slot(11).unwrap().count, 3);
    assert_eq!(inventory.count(ItemKind::Block(3)), 12);
    assert_eq!(inventory.count(ItemKind::Block(2)), 5);
    assert_eq!(inventory.count(ItemKind::Block(4)), 3);
}

#[test]
fn empty_slot_click_and_q_are_silent_but_a_queued_fill_then_drop_still_executes() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Drop(false));
    assert!(requests(&mut app).is_empty());
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, pointer_action("hotbar", Some(0), "", false));
    assert!(requests(&mut app).is_empty());
    assert!(app.world().resource::<InventoryView>().error.is_empty());
    action(&mut app, InventoryAction::Close);
    app.update();
    action(&mut app, InventoryAction::PickBlock(3));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::CreativeSet {
            slot: 0,
            item: ItemKind::Block(3),
            count: 1
        }
    );
    action(&mut app, InventoryAction::Drop(false));
    assert!(requests(&mut app).is_empty());
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.insert(ItemKind::Block(3), 1);
        session.inventory_response += 1;
    }
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::DropSlot {
            slot: 0,
            all: false
        }
    );
}

#[test]
fn legacy_catalog_selection_of_grass_survives_the_initial_session_ready_transition() {
    let mut app = app(GameMode::Creative);
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .choices
        .push(BlockChoice {
            id: 1,
            name: "Grass".into(),
            identifier: "teramic::grass".into(),
            description: "Fixture registry grass".into(),
        });
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    app.update();
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Select("block-1".into()));
    let commands = requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].action,
        ServerAction::CreativeSet {
            slot: 0,
            item: ItemKind::Block(1),
            count: 1
        }
    );
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        assert!(session.inventory.creative_set(
            0,
            terra_core::ItemStack {
                item: ItemKind::Block(1),
                count: 128
            }
        ));
        session.inventory_response += 1;
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<BlockPalette>()
            .selected()
            .unwrap()
            .id,
        1
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(ItemKind::Block(1)),
        128
    );
}

#[test]
fn an_exhausted_right_sweep_never_picks_up_half_of_a_later_occupied_slot() {
    let mut app = app(GameMode::Survival);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.slots = vec![
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 5,
            },
            terra_core::ItemStack {
                item: ItemKind::Block(3),
                count: 7,
            },
        ];
        session.inventory.cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        });
    }
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    action(&mut app, pointer_action("hotbar", Some(0), "block-3", true));
    hover_slot(&mut app, 1, "block-3");
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Click {
            slot: 0,
            secondary: true
        }
    );
    assert!(requests(&mut app).is_empty());
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert!(inventory.cursor.is_none());
    assert_eq!(inventory.slot(0).unwrap().count, 6);
    assert_eq!(inventory.slot(1).unwrap().count, 7);
    assert_eq!(inventory.count(ItemKind::Block(3)), 13);
}

#[test]
fn an_exhausted_cursor_wheel_batch_never_turns_into_a_backpack_transfer() {
    let mut app = app(GameMode::Survival);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session
            .inventory
            .slots
            .resize(10, terra_core::ItemStack::EMPTY);
        session.inventory.slots[0] = terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 5,
        };
        session.inventory.slots[9] = terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 10,
        };
        session.inventory.cursor = Some(terra_core::ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        });
    }
    action(&mut app, InventoryAction::Toggle);
    hover_slot(&mut app, 0, "block-3");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    app.world_mut().write_message(MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: -3.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    assert_eq!(
        apply_pointer_ack(&mut app),
        ServerAction::Wheel {
            slot: 0,
            to_cursor: false
        }
    );
    assert!(requests(&mut app).is_empty());
    let inventory = &app.world().resource::<ClientSession>().inventory;
    assert!(inventory.cursor.is_none());
    assert_eq!(inventory.slot(0).unwrap().count, 6);
    assert_eq!(inventory.slot(9).unwrap().count, 10);
    assert_eq!(inventory.count(ItemKind::Block(3)), 16);
}
#[test]
fn browser_pagination_uses_all_complete_rows_at_current_scale() {
    assert_eq!(
        browser_capacity(Vec2::new(605.0, 605.0), Vec2::splat(56.0), Vec2::splat(5.0)),
        100
    );
    assert_eq!(
        browser_capacity(
            Vec2::new(605.0, 605.0) * 1.5,
            Vec2::splat(84.0),
            Vec2::splat(7.5)
        ),
        100
    );
    assert_eq!(
        browser_capacity(Vec2::new(117.0, 56.0), Vec2::splat(56.0), Vec2::splat(5.0)),
        2
    );
}

fn mixer_snapshot(anchor: [i64; 3], target: [u8; 3]) -> ContainerSnapshot {
    ContainerSnapshot {
        anchor,
        workbench: None,
        view: ContainerView::Primary,
        attached_campfire: false,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        furnace: None,
        crusher: None,
        mixer: Some(terra_core::mixer::MixerView {
            target,
            progress: 0.25,
            working: true,
        }),
        filter: None,
        distributor: None,
        halves: vec![ContainerHalf {
            position: anchor,
            inventory: terra_core::Inventory {
                slots: vec![terra_core::ItemStack::EMPTY; terra_core::mixer::SLOTS],
                ..default()
            },
        }],
    }
}

#[test]
fn mixer_uses_seven_real_slots_recipe_ghosts_and_live_kinetic_progress() {
    let mut app = app(GameMode::Survival);
    let anchor = [-4, 60, 8];
    let target = [20, 115, 210];
    let mut snapshot = mixer_snapshot(anchor, target);
    let output = terra_core::mixer::recipe(target).output;
    snapshot.halves[0].inventory.slots[terra_core::mixer::OUTPUT] = output;
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.container = Some(snapshot);
        session.kinetics = vec![terra_network::protocol::session::KineticVisual {
            position: anchor,
            rpm: 7.5,
            ..default()
        }];
    }
    app.update();
    let before = app.world().resource::<ClientSession>().container.clone();
    let view = app.world().resource::<InventoryView>();
    assert!(view.mixer && !view.container && !view.crusher && !view.furnace);
    assert_eq!(view.title, "Mixer");
    assert_eq!(view.mixer_color, "#1473D2");
    assert_eq!(view.container_items.len(), terra_core::mixer::SLOTS);
    assert_eq!(view.mixer_input.len(), terra_core::mixer::INPUT_SLOTS);
    assert!(
        view.mixer_input
            .iter()
            .all(|slot| slot.empty && slot.item_id.is_empty())
    );
    assert!(
        view.mixer_input
            .iter()
            .any(|slot| !slot.ghost_preview.is_empty())
    );
    assert_eq!(view.mixer_output[0].index, "6");
    assert_eq!(view.mixer_output[0].preview_key, preview_key(output.item));
    assert_eq!(view.mixer_progress_style, "width: 25.0%;");
    assert_eq!(view.mixer_status, "7.5 RPM - 25%");
    assert!(view.mixer_working);
    app.world_mut().resource_mut::<ClientSession>().kinetics[0].overloaded = true;
    app.world_mut().resource_mut::<ClientSession>().kinetics[0].rpm = 0.0;
    app.update();
    assert_eq!(
        app.world().resource::<InventoryView>().mixer_status,
        "Overloaded"
    );
    assert!(!app.world().resource::<InventoryView>().mixer_working);
    assert_eq!(app.world().resource::<ClientSession>().container, before);
}

#[test]
fn mixer_color_queue_is_byte_exact_coalesces_picker_changes_and_rejects_wrong_owner() {
    let mut app = app(GameMode::Survival);
    let anchor = [2, 70, -3];
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(mixer_snapshot(anchor, [1, 2, 3]));
    action(&mut app, InventoryAction::Toggle);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    action(&mut app, InventoryAction::MixerColor("#123456".into()));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetMixerColor {
            station: anchor,
            target: [18, 52, 86]
        }
    );
    assert_eq!(
        app.world().resource::<InventoryView>().mixer_color,
        "#123456"
    );
    for value in ["#123456", "#654321", "#ABCDEF"] {
        app.world_mut()
            .write_message(InventoryAction::MixerColor(value.into()));
    }
    app.update();
    assert!(requests(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<InventoryView>().mixer_color,
        "#ABCDEF"
    );
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session
            .container
            .as_mut()
            .unwrap()
            .mixer
            .as_mut()
            .unwrap()
            .target = [18, 52, 86];
    }
    app.update();
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetMixerColor {
            station: anchor,
            target: [171, 205, 239]
        }
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([9, 70, -3], false));
    action(&mut app, InventoryAction::MixerColor("#000000".into()));
    assert!(requests(&mut app).is_empty());
}

#[test]
fn mixed_dyes_have_unique_hex_metadata_without_expanding_the_creative_catalog() {
    use terra_core::beds::DyeColor;
    let palette = BlockPalette::default();
    let a = ItemKind::Dye(DyeColor::Rgb([1, 2, 3]));
    let b = ItemKind::Dye(DyeColor::Rgb([1, 2, 4]));
    assert_ne!(item_id(a), item_id(b));
    let view = view::item_view(a, 16, &palette);
    assert_eq!(view.name, "Mixed Dye #010203");
    assert!(view.identifier.ends_with("mixed_dye#010203"));
    assert_eq!(view.description, "RGB: 1, 2, 3");
    assert!(!catalog_item(a));
    assert!(
        !available_items(
            GameMode::Creative,
            &terra_core::Inventory::default(),
            &palette
        )
        .contains(&a)
    );
    for color in DyeColor::ALL {
        assert!(catalog_item(ItemKind::Dye(color)));
    }
}

#[test]
fn rgb_recipe_inspection_is_on_demand_and_can_only_set_a_real_mixers_target() {
    use terra_core::{ItemStack, beds::DyeColor};
    let target = [71, 93, 184];
    let item = ItemKind::Dye(DyeColor::Rgb(target));
    let recipe = terra_core::mixer::recipe(target);
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(0, ItemStack { item, count: 1 });
    let book_size = app
        .world()
        .resource::<terra_core::crafting::RecipeBook>()
        .iter()
        .count();
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Recipe(item_id(item)));
    let view = app.world().resource::<CraftingView>();
    assert!(view.recipe_open && view.selected && view.mixing && !view.smelting && !view.crushing);
    assert_eq!(view.recipe_station, "Mixer");
    assert_eq!(view.recipe_grid.len(), recipe.inputs.len());
    for (slot, required) in view.recipe_grid.iter().zip(&recipe.inputs) {
        assert_eq!(slot.preview_key, preview_key(required.item));
        assert_eq!(slot.count, required.count.to_string());
    }
    assert_eq!(view.recipe_output[0].preview_key, preview_key(item));
    assert_eq!(view.recipe_output[0].count, recipe.output.count.to_string());
    assert!(!view.can_apply_recipe);
    add_recipe(&mut app);
    assert!(requests(&mut app).is_empty());
    let anchor = [-2, 70, 4];
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(mixer_snapshot(anchor, [1, 2, 3]));
    action(&mut app, InventoryAction::Recipe(item_id(item)));
    let before = app.world().resource::<ClientSession>().inventory.clone();
    let before_machine = app.world().resource::<ClientSession>().container.clone();
    assert!(app.world().resource::<CraftingView>().can_apply_recipe);
    assert_eq!(
        app.world().resource::<CraftingView>().apply_title,
        "Set mixer target"
    );
    add_recipe(&mut app);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetMixerColor {
            station: anchor,
            target
        }
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert_eq!(
        app.world().resource::<ClientSession>().container,
        before_machine
    );
    assert_eq!(
        app.world()
            .resource::<terra_core::crafting::RecipeBook>()
            .iter()
            .count(),
        book_size
    );
    assert!(
        !app.world()
            .resource::<InventoryView>()
            .browser_items
            .iter()
            .any(|slot| slot.item_id == item_id(item))
    );
    // Applying the valid mixer target closes the recipe before this invalid request.
    assert!(!app.world().resource::<CraftingView>().recipe_open);
    let before_view =
        tilt_ui::serde_json::to_value(app.world().resource::<CraftingView>()).unwrap();
    action(
        &mut app,
        InventoryAction::Recipe(item_id(ItemKind::Dye(DyeColor::Rgb([20, 30, 40])))),
    );
    // This second RGB item is not present in any inspectable inventory or catalog.
    // Its invalid request cannot reopen or change the closed view or enqueue a target.
    let view = app.world().resource::<CraftingView>();
    assert_eq!(tilt_ui::serde_json::to_value(view).unwrap(), before_view);
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert_eq!(
        app.world().resource::<ClientSession>().container,
        before_machine
    );
    assert!(requests(&mut app).is_empty());
}

#[test]
fn recipe_shortcut_can_inspect_a_real_rgb_stack_outside_the_creative_catalog() {
    use terra_core::{ItemStack, beds::DyeColor};
    let item = ItemKind::Dye(DyeColor::Rgb([71, 93, 184]));
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(0, ItemStack { item, count: 1 });
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        InventoryAction::Hover {
            command: "hotbar".into(),
            index: Some(0),
            item: item_id(item),
        },
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert!(app.world().resource::<CraftingView>().mixing);
    assert_eq!(
        app.world().resource::<CraftingView>().recipe_output[0].preview_key,
        preview_key(item)
    );
}

#[test]
fn closing_a_machine_during_teleport_preserves_the_real_input_capture_handoff() {
    let mut app = app(GameMode::Survival);
    app.world_mut()
        .resource_mut::<ControlsConfig>()
        .rebind(GameAction::Place, InputBinding::Mouse(MouseButton::Right))
        .unwrap();
    app.init_resource::<terra_network::client::ClientChat>()
        .init_resource::<terra_network::client::ClientChatCommands>();
    crate::chat::install(&mut app);
    let anchor = [2, 17, 0];
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(mixer_snapshot(anchor, [71, 93, 184]));
    action(&mut app, InventoryAction::Toggle);
    app.update();
    action(&mut app, InventoryAction::Close);
    assert!(matches!(
        requests(&mut app).as_slice(),
        [InventoryCommand {
            action: ServerAction::CloseContainer,
            ..
        }]
    ));
    assert!(app.world().resource::<InventoryState>().resume_pending);
    app.world_mut()
        .resource_mut::<Messages<CaptureRequest>>()
        .clear();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.teleporting = true;
        session.container = None;
        session.inventory_response += 1;
    }
    for _ in 0..4 {
        app.update();
        assert!(!app.world().resource::<InventoryState>().open);
        assert!(app.world().resource::<InventoryState>().resume_pending);
        assert!(app.world().resource::<PlayerInputBlock>().0);
        assert!(
            app.world_mut()
                .resource_mut::<Messages<CaptureRequest>>()
                .drain()
                .all(|request| !request.0)
        );
    }
    app.world_mut().resource_mut::<ClientSession>().teleporting = false;
    app.world_mut().resource_mut::<crate::SettingsView>().open = true;
    app.update();
    assert!(app.world().resource::<InventoryState>().resume_pending);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .all(|request| !request.0)
    );
    app.world_mut().resource_mut::<crate::SettingsView>().open = false;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    assert!(app.world().resource::<InventoryState>().resume_pending);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .all(|request| !request.0)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);
    app.update();
    assert!(!app.world().resource::<InventoryState>().blocks_input());
    assert!(!app.world().resource::<PlayerInputBlock>().0);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .filter(|request| request.0)
            .count(),
        1
    );
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<PlayerInputBlock>().0);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .next()
            .is_none()
    );
    // The server may begin a later teleport after closing has already fully resumed.
    app.world_mut().resource_mut::<ClientSession>().teleporting = true;
    app.update();
    assert!(!app.world().resource::<InventoryState>().resume_pending);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut().resource_mut::<ClientSession>().teleporting = false;
    app.update();
    assert!(!app.world().resource::<PlayerInputBlock>().0);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .filter(|request| request.0)
            .count(),
        1
    );
}

#[test]
fn faucet_mouse_buttons_send_one_directional_step_and_shift_keeps_removal_available() {
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};
    for (button, shift, expected) in [
        (MouseButton::Left, false, Some(true)),
        (MouseButton::Right, false, Some(false)),
        (MouseButton::Left, true, None),
        (MouseButton::Right, true, None),
    ] {
        let mut app = food_input_app(InputBinding::Mouse(MouseButton::Right));
        app.world_mut().resource_mut::<ClientSession>().allow_build = true;
        let position = [1, 1, 1];
        let section = SectionPos::default();
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([1, 1, 1], 167).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(BlockHit {
                    position,
                    normal: [0, 0, 1],
                    distance: 2.0,
                }),
            )));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(button);
        if shift {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ShiftLeft);
        }
        app.update();
        let events: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<terra_network::client::ObjectInteractionRequest>>()
            .drain()
            .collect();
        assert_eq!(events.len(), usize::from(expected.is_some()));
        if let Some(closing) = expected {
            assert_eq!(events[0].expected, 167);
            assert_eq!(events[0].closing, closing);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Messages<terra_network::client::ObjectInteractionRequest>>()
                .drain()
                .next()
                .is_none(),
            "holding a button cannot advance more stages"
        );
        assert!(!app.world().resource::<InventoryState>().open);
    }
}

#[test]
fn filled_cans_retain_identity_in_slots_and_inspection_while_sharing_only_the_tool_icon() {
    let mut palette = BlockPalette::default();
    let tool = terra_core::ToolKind::IronWateringCan;
    let empty = ItemKind::Tool(tool);
    let first = empty.with_watering_units(2).unwrap();
    let second = empty.with_watering_units(3).unwrap();
    palette.item_metadata.insert(
        empty,
        ("Iron Watering Can".into(), "Reusable watering tool".into()),
    );
    palette.item_categories.insert(empty, "tools".into());
    let view = view::item_view(first, 1, &palette);
    assert_eq!(view.item, Some(first));
    assert_eq!(view.identifier, first.identifier());
    assert!(view.name.starts_with("Iron Watering Can"));
    assert!(
        view.description
            .contains(&format!("Wasser: 2 / {}", tool.watering_capacity()))
    );
    assert_eq!(view.category, "tools");
    assert_ne!(item_id(first), item_id(second));
    assert_ne!(item_id(first), item_id(empty));
    assert_eq!(preview_key(first), preview_key(empty));
    assert_eq!(preview_key(second), preview_key(empty));
    let mut session = ClientSession::default();
    session.inventory.creative_set(
        0,
        terra_core::ItemStack {
            item: first,
            count: 1,
        },
    );
    session.inventory.creative_set(
        1,
        terra_core::ItemStack {
            item: second,
            count: 1,
        },
    );
    assert_eq!(
        inspectable_item(&session, &palette, &item_id(first)),
        Some(first)
    );
    assert_eq!(
        inspectable_item(&session, &palette, &item_id(second)),
        Some(second)
    );
}

#[test]
fn inventory_book_collapses_the_catalog_and_both_close_keys_dismiss_the_popup_first() {
    for key in [KeyCode::KeyE, KeyCode::Escape] {
        let mut app = app(GameMode::Creative);
        action(&mut app, InventoryAction::Toggle);
        assert!(app.world().resource::<InventoryView>().browser_visible);
        let baseline = app.world().resource::<ClientSession>().inventory.clone();
        action(&mut app, InventoryAction::ToggleBrowser);
        assert!(!app.world().resource::<InventoryView>().browser_visible);
        assert!(!app.world().resource::<CraftingView>().recipe_open);
        action(&mut app, InventoryAction::ToggleBrowser);
        assert!(app.world().resource::<InventoryView>().browser_visible);
        app.world_mut()
            .write_message(crate::CraftingAction::OpenBook);
        app.update();
        assert!(app.world().resource::<CraftingView>().recipe_open);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        assert!(!app.world().resource::<CraftingView>().recipe_open);
        assert!(app.world().resource::<InventoryView>().open);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        assert!(!app.world().resource::<InventoryView>().open);
        assert_eq!(app.world().resource::<ClientSession>().inventory, baseline);
    }
}

#[test]
fn catalog_page_buttons_stop_at_bounds_and_search_normalizes_the_internal_page() {
    let mut app = app(GameMode::Creative);
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<InventoryState>()
        .page_capacity = 2;
    app.update();
    let pages = app.world().resource::<InventoryView>().browser_pages;
    assert!(pages > 1);
    for _ in 0..pages + 3 {
        action(&mut app, InventoryAction::Page(1));
    }
    assert_eq!(app.world().resource::<InventoryState>().page, pages - 1);
    assert!(!app.world().resource::<InventoryView>().browser_next);
    action(&mut app, InventoryAction::Page(-1));
    assert_eq!(app.world().resource::<InventoryState>().page, pages - 2);
    action(&mut app, InventoryAction::Search("does-not-exist".into()));
    action(&mut app, InventoryAction::Page(1));
    assert_eq!(app.world().resource::<InventoryState>().page, 0);
    assert_eq!(
        app.world().resource::<InventoryView>().browser_page,
        "1 / 1"
    );
}

#[test]
fn explicit_catalog_tracks_ignore_pixel_rounding_and_resize_keeps_the_visible_item_offset() {
    let mut app = App::new();
    app.insert_resource(InventoryState {
        open: true,
        page: 2,
        page_capacity: 70,
        ..default()
    })
    .add_systems(Update, browser_layout);
    let grid = app
        .world_mut()
        .spawn((
            ElementId("inventory-browser-grid".into()),
            ComputedNode {
                size: Vec2::new(368.0, 427.0),
                ..default()
            },
            Node {
                grid_template_columns: vec![RepeatedGridTrack::fr(7, 1.0)],
                grid_template_rows: vec![RepeatedGridTrack::fr(10, 1.0)],
                column_gap: Val::Px(3.0),
                row_gap: Val::Px(3.0),
                ..default()
            },
        ))
        .id();
    let cell = app
        .world_mut()
        .spawn((
            ElementId("inventory-cell".into()),
            ComputedNode {
                size: Vec2::new(50.1, 40.1),
                ..default()
            },
            Node::default(),
            ChildOf(grid),
        ))
        .id();
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().page_capacity, 70);
    assert_eq!(app.world().resource::<InventoryState>().page, 2);
    app.world_mut().get_mut::<ComputedNode>(cell).unwrap().size = Vec2::new(49.9, 39.9);
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().page_capacity, 70);
    assert_eq!(app.world().resource::<InventoryState>().page, 2);
    app.world_mut()
        .get_mut::<ComputedNode>(grid)
        .unwrap()
        .size
        .y = 212.0;
    app.update();
    assert_eq!(app.world().resource::<InventoryState>().page_capacity, 35);
    assert_eq!(app.world().resource::<InventoryState>().page, 4);
}
