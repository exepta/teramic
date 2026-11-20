use super::*;
use terra_core::{
    PlayerUuid,
    building_jobs::{HOUSE_TEMPLATE, SettlementBuilding, template::TemplateCatalog},
};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Time<Real>>()
        .init_resource::<crate::npc_work::WorkUiState>()
        .init_resource::<terra_player::WorldInteractionBlock>()
        .init_resource::<terra_player::PlayerInputBlock>()
        .add_message::<terra_player::CaptureRequest>()
        .add_message::<terra_network::client::NpcWorkRequest>();
    let catalog =
        TemplateCatalog::load_directory(&terra_core::asset_directory().join("buildings/templates"))
            .unwrap();
    let mut session = ClientSession::default();
    session.ready = true;
    session.buildings.templates = catalog.summaries();
    let mut site = SettlementBuilding::new(PlayerUuid::from_u128(240));
    site.storage = Some(BuildArea::new([0, 1, 0], [2, 2, 2]).unwrap());
    session.buildings.settlements.insert(KingdomId(1), site);
    app.insert_resource(session)
        .insert_resource(crate::KingdomView {
            visible: true,
            bound: true,
            building_tab: true,
            manager_controls: true,
            settlement_id: 1,
            ..Default::default()
        });
    install(&mut app);
    app.update();
    app.world_mut()
        .resource_mut::<Messages<BuildingRequest>>()
        .clear();
    app
}

#[test]
fn selecting_a_real_template_changes_exact_material_preview_without_submitting_or_editing_saved_choice()
 {
    let mut app = app();
    let expected = app
        .world()
        .resource::<ClientSession>()
        .buildings
        .templates
        .iter()
        .find(|template| template.id == "spruce-cottage-v1")
        .unwrap()
        .materials
        .clone();
    app.world_mut()
        .write_message(BuildingUiAction::Template("spruce-cottage-v1".into()));
    app.update();
    let view = app.world().resource::<BuildingView>();
    assert!(view.template_selected);
    assert_eq!(
        view.templates
            .iter()
            .filter(|template| template.selected)
            .count(),
        1
    );
    assert_eq!(view.template_name, "Spruce cottage");
    assert_eq!(view.template_materials.len(), expected.len());
    for (row, stack) in view.template_materials.iter().zip(expected) {
        assert_eq!(row.identifier, stack.item.identifier());
        assert_eq!(row.count, stack.count.to_string());
        assert_eq!(row.preview_key, crate::inventory::preview_key(stack.item));
    }
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .buildings
            .settlements[&KingdomId(1)]
            .selected_template,
        HOUSE_TEMPLATE
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<BuildingRequest>>()
            .drain()
            .count(),
        0
    );
}

#[test]
fn manual_and_automatic_commands_use_the_explicit_preview_choice() {
    for automatic in [false, true] {
        let mut app = app();
        app.world_mut()
            .write_message(BuildingUiAction::Template("spruce-cottage-v1".into()));
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<BuildingUiState>();
            state.origin = Some([-24, 4, -24]);
            state.residential = Some(BuildArea::new([-32, 4, -32], [-1, 6, -1]).unwrap());
        }
        app.world_mut().write_message(if automatic {
            BuildingUiAction::SaveResidential
        } else {
            BuildingUiAction::QueueHouse
        });
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BuildingRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        match &requests[0].0.action {
            BuildingAction::SetResidential { template, area, .. } if automatic => {
                assert_eq!(template, "spruce-cottage-v1");
                assert!(area.is_some());
            }
            BuildingAction::QueueHouse {
                template, origin, ..
            } if !automatic => {
                assert_eq!(template, "spruce-cottage-v1");
                assert_eq!(*origin, [-24, 4, -24]);
            }
            other => panic!("Wrong selected-template command: {other:?}"),
        }
    }
}

#[test]
fn removed_catalog_selection_is_not_replaced_silently_and_its_automatic_area_can_still_be_stopped()
{
    let mut app = app();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        let site = session
            .buildings
            .settlements
            .get_mut(&KingdomId(1))
            .unwrap();
        site.selected_template = "removed-residential-v1".into();
        site.residential = Some(BuildArea::new([0, 1, 0], [20, 1, 20]).unwrap());
    }
    app.update();
    assert!(!app.world().resource::<BuildingView>().template_selected);
    assert!(!app.world().resource::<BuildingView>().can_queue);
    assert!(
        !app.world()
            .resource::<BuildingView>()
            .template_error
            .is_empty()
    );
    app.world_mut()
        .write_message(BuildingUiAction::DisableResidential);
    app.update();
    assert!(app.world_mut().resource_mut::<Messages<BuildingRequest>>().drain().any(|request| {
        matches!(request.0.action,BuildingAction::SetResidential { area:None,template,.. } if template=="removed-residential-v1")
    }));
}

#[test]
fn unknown_local_card_id_does_not_change_selection_or_send_any_request() {
    let mut app = app();
    app.world_mut()
        .write_message(BuildingUiAction::Template("../../outside".into()));
    app.update();
    assert_eq!(
        app.world().resource::<BuildingUiState>().template,
        HOUSE_TEMPLATE
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<BuildingRequest>>()
            .drain()
            .count(),
        0
    );
}

#[test]
fn real_spruce_material_names_reach_both_locale_projections_without_raw_wood_slugs() {
    use terra_core::{GameLanguage, GameplayConfig};
    use terra_player::interaction::{BlockChoice, BlockPalette};
    let mut app = app();
    let mut palette = BlockPalette {
        choices: Vec::new(),
        ..Default::default()
    };
    for file in ["spruce-planks.json", "spruce-log.json"] {
        let path = terra_core::asset_directory()
            .join("terra/blocks")
            .join(file);
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        palette.choices.push(BlockChoice {
            id: value["id"].as_u64().unwrap() as u16,
            name: value["name"].as_str().unwrap().into(),
            identifier: terra_core::ItemKind::Block(value["id"].as_u64().unwrap() as u16)
                .identifier(),
            description: value["description"].as_str().unwrap_or_default().into(),
        });
    }
    app.insert_resource(palette);
    crate::localization::install(&mut app);
    let mut localization = tilt_ui::UiLocalization::new("en-US").unwrap();
    localization
        .insert_ftl(
            "de-DE",
            include_str!("../../../../src-ui/locales/de-DE.ftl"),
        )
        .unwrap();
    localization
        .insert_ftl(
            "en-US",
            include_str!("../../../../src-ui/locales/en-US.ftl"),
        )
        .unwrap();
    app.insert_resource(localization);
    app.world_mut()
        .write_message(BuildingUiAction::Template("spruce-cottage-v1".into()));
    for (language, expected) in [
        (GameLanguage::German, ["Fichtenbretter", "Fichtenstamm"]),
        (GameLanguage::English, ["Spruce Planks", "Spruce Log"]),
    ] {
        app.world_mut().resource_mut::<GameplayConfig>().language = language;
        app.update();
        app.update();
        let view = app.world().resource::<crate::localization::LocalizedUi>();
        let rows = view.roots["buildingView"]["template_materials"]
            .as_array()
            .unwrap();
        for label in expected {
            assert!(
                rows.iter().any(|row| row["label"] == label),
                "Missing {label}: {rows:?}"
            );
        }
    }
}
