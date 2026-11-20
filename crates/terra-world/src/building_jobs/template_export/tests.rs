use super::*;
use terra_core::building_jobs::template::DEFAULT_HOUSE_PATH;

#[test]
fn actual_json_default_matches_the_existing_village_house_in_all_four_orientations() {
    let exported = export_default_house_template().unwrap();
    let loaded =
        BuildingTemplate::load(&terra_core::asset_directory().join(DEFAULT_HOUSE_PATH)).unwrap();
    assert_eq!(loaded, exported);
    for facing in 0..4 {
        let origin = [-33, 12, -1];
        assert_eq!(
            loaded.placed(origin, facing).unwrap(),
            captured(origin, facing).unwrap()
        );
    }
}

#[test]
fn runtime_preview_uses_the_same_loaded_json_and_has_no_free_loot_or_residents() {
    let template =
        BuildingTemplate::load(&terra_core::asset_directory().join(DEFAULT_HOUSE_PATH)).unwrap();
    assert_eq!(
        super::super::preview_house().unwrap(),
        template.placed([0; 3], 0).unwrap()
    );
    assert!(template.steps.iter().all(|step| !step.costs.is_empty()));
    assert!(
        template
            .steps
            .iter()
            .filter(|step| step.cells[0].material == 29)
            .all(|step| step.costs.len() == 1)
    );
}
