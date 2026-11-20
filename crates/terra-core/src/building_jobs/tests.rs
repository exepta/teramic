use super::*;

#[test]
fn crew_rates_match_the_requested_real_time_and_reject_more_than_four() {
    for (workers, seconds) in [(1, 1800.0), (2, 900.0), (3, 600.0), (4, 300.0)] {
        assert_eq!(house_seconds(workers), Some(seconds));
        assert_eq!(seconds * 1800.0 / house_seconds(workers).unwrap(), 1800.0);
    }
    assert!(house_seconds(0).is_none());
    assert!(house_seconds(5).is_none());
}
#[test]
fn areas_are_negative_safe_bounded_and_independent_of_corner_order() {
    let a = BuildArea::new([-1, -3, -2], [-16, 12, -31]).unwrap();
    assert_eq!(a, BuildArea::new([-16, 12, -31], [-1, -3, -2]).unwrap());
    assert!(!a.contains([-1, 0, -1]));
    assert_eq!(a.cell_at(0), Some(a.min));
    assert_eq!(a.cell_at(a.volume() - 1), Some(a.max));
    assert_eq!(a.cell_at(a.volume()), None);
    assert!(BuildArea::new([0; 3], [64, 1, 1]).is_err());
    assert!(BuildingRegistry::default().valid());
}

#[test]
fn legacy_json_settlements_default_the_template_and_reject_unsafe_ids_without_catalog_dependency() {
    let original = SettlementBuilding::new(crate::PlayerUuid::from_u128(240));
    let mut value = serde_json::to_value(&original).unwrap();
    value.as_object_mut().unwrap().remove("selected_template");
    let mut loaded: SettlementBuilding = serde_json::from_value(value).unwrap();
    assert_eq!(loaded, original);
    loaded.selected_template = "removed-house-v1".into();
    assert!(
        loaded.valid(),
        "existing saves remain valid independently of installed assets"
    );
    loaded.selected_template = "../arbitrary.json".into();
    assert!(!loaded.valid());
}

#[test]
fn town_totals_and_server_preview_are_bounded_and_validate_real_template_cells() {
    use crate::{ItemKind, MineralItem};
    let mut stock = TownStock {
        configured: true,
        complete: true,
        chests: 2,
        items: std::collections::BTreeMap::from([
            (ItemKind::Block(16), 200),
            (ItemKind::Mineral(MineralItem::Coal), 7),
        ]),
        ..Default::default()
    };
    stock.summarize();
    assert!(stock.valid());
    assert_eq!((stock.wood, stock.coal), (200, 7));
    stock.items.insert(ItemKind::Block(16), u32::MAX);
    assert!(!stock.valid());
    let source = template::BuildingTemplate::load(
        &crate::asset_directory().join(template::DEFAULT_HOUSE_PATH),
    )
    .unwrap();
    let preview = TemplatePreview::from_template(&source).unwrap();
    assert!(preview.valid());
    assert_eq!(
        preview.cells.len(),
        source.steps.iter().map(|s| s.cells.len()).sum::<usize>()
    );
    let mut invalid = preview.clone();
    invalid.cells.push(invalid.cells[0].clone());
    assert!(!invalid.valid());
    invalid = preview.clone();
    invalid.cells[0].position = [127, 0, 0];
    assert!(!invalid.valid());
    invalid = preview;
    invalid.cells[0].facing = 255;
    assert!(!invalid.valid());
}
