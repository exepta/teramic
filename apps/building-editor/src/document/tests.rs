use super::*;
use terra_core::{
    ItemKind,
    building_jobs::{BuildPhase, template::BuildingTemplate},
};

fn cell(material: u16) -> AuthoredCell {
    AuthoredCell {
        material,
        state: Default::default(),
    }
}
#[test]
fn new_document_has_no_exported_support_voxels() {
    let document = Document::default();
    assert!(document.cells.is_empty());
    assert!(!document.dirty());
    assert!(document.template().is_err());
    let journal =
        crate::scene::journal(&document, crate::workspace::WorkspacePlatform::default()).unwrap();
    assert_eq!(journal.edited_blocks(), 81);
    assert_eq!(journal.block([0, PLATFORM_Y, 0]), Some(43));
}
#[test]
fn ordinary_native_materials_use_an_implicit_pose_but_invalid_furniture_does_not() {
    let mut document = Document::default();
    for (x, material) in [3, 16, 17, 18, 36, 43].into_iter().enumerate() {
        document.cells.insert([x as i64 - 3, -1, 0], cell(material));
    }
    let restored = Document::from_template(document.template().unwrap(), None).unwrap();
    assert_eq!(restored.cells, document.cells);
    let journal =
        crate::scene::journal(&restored, crate::workspace::WorkspacePlatform::default()).unwrap();
    for position in restored.cells.keys() {
        assert!(journal.object_state(*position).is_none());
    }
    document.cells.insert([0, 0, 0], cell(65));
    assert!(
        document.template().is_err(),
        "An ordinary pose cannot substitute a missing door pair"
    );
}
#[test]
fn all_native_carpets_roundtrip_without_an_invented_object_state() {
    // Shared placement deliberately handles these as supported, stateless
    // decorations even though furniture() includes them for targeting.
    for material in terra_core::textiles::WHITE_CARPET..=terra_core::textiles::LAST_CARPET {
        let mut document = Document::default();
        document.cells.insert([0, -1, 0], cell(43));
        document.cells.insert([0, 0, 0], cell(material));
        let template = document.template().unwrap();
        let carpet = template
            .steps
            .iter()
            .flat_map(|step| &step.cells)
            .find(|cell| cell.material == material)
            .unwrap();
        assert!(!carpet.rotate_facing);
        let restored = Document::from_template(template, None).unwrap();
        assert_eq!(restored.cells, document.cells);
        let journal =
            crate::scene::journal(&restored, crate::workspace::WorkspacePlatform::default())
                .unwrap();
        assert_eq!(journal.block([0, 0, 0]), Some(material));
        assert!(journal.object_state([0, 0, 0]).is_none());
        document.cells.get_mut(&[0, 0, 0]).unwrap().state.facing = 1;
        assert!(
            document.template().is_err(),
            "Stateless carpet cannot preserve a fabricated pose"
        );
    }
}
#[test]
fn exact_default_house_can_be_imported_modified_and_reloaded() {
    let original = BuildingTemplate::load(
        &terra_core::asset_directory()
            .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH),
    )
    .unwrap();
    let mut document = Document::from_template(original.clone(), None).unwrap();
    assert_eq!(
        document.cells.len(),
        original.steps.iter().map(|s| s.cells.len()).sum::<usize>()
    );
    let journal =
        crate::scene::journal(&document, crate::workspace::WorkspacePlatform::default()).unwrap();
    for (&p, cell) in &document.cells {
        assert_eq!(journal.block(p), Some(cell.material));
        assert_eq!(
            journal.object_state(p).unwrap_or_default().facing,
            cell.state.facing
        );
    }
    document.cells.insert([4, 8, 4], cell(43));
    let result = document.template().unwrap();
    assert_eq!(
        result.steps.iter().map(|s| s.cells.len()).sum::<usize>(),
        document.cells.len()
    );
    assert!(
        result
            .steps
            .iter()
            .flat_map(|s| &s.cells)
            .any(|c| c.position == [4, 8, 4])
    );
    assert_eq!(
        BuildingTemplate::from_json(&serde_json::to_string(&result).unwrap()).unwrap(),
        result
    );
}
#[test]
fn phases_costs_and_paired_beds_are_exact() {
    let mut document = Document::default();
    document.cells.insert([0, -2, 0], cell(43));
    document.cells.insert([0, -1, 0], cell(16));
    document.cells.insert([0, 0, 0], cell(43));
    document.cells.insert([0, 5, 0], cell(16));
    document.cells.insert(
        [1, 0, 1],
        AuthoredCell {
            material: 71,
            state: BlockObjectState {
                peer: Some([1, 0, 2]),
                ..Default::default()
            },
        },
    );
    document.cells.insert(
        [1, 0, 2],
        AuthoredCell {
            material: 71,
            state: BlockObjectState {
                peer: Some([1, 0, 1]),
                secondary: true,
                ..Default::default()
            },
        },
    );
    let template = document.template().unwrap();
    assert_eq!(
        template.steps.iter().map(|s| s.phase).collect::<Vec<_>>(),
        [
            BuildPhase::Foundation,
            BuildPhase::Floor,
            BuildPhase::Walls,
            BuildPhase::Roof,
            BuildPhase::Furnishing
        ]
    );
    assert_eq!(
        template
            .materials
            .iter()
            .find(|s| s.item == ItemKind::Block(71))
            .unwrap()
            .count,
        1
    );
    assert_eq!(template.steps.last().unwrap().cells.len(), 2);
}
#[test]
fn broken_pairs_outside_bounds_and_lossy_states_are_rejected() {
    let mut document = Document::default();
    document.cells.insert(
        [0, 0, 0],
        AuthoredCell {
            material: 71,
            state: BlockObjectState {
                peer: Some([0, 0, 1]),
                ..Default::default()
            },
        },
    );
    assert!(document.template().is_err());
    document.cells.clear();
    document.cells.insert([5, 0, 0], cell(3));
    assert!(document.template().is_err());
    document.cells.clear();
    document.cells.insert(
        [0, 0, 0],
        AuthoredCell {
            material: 29,
            state: BlockObjectState {
                wood: terra_core::WoodSpecies::Birch,
                ..Default::default()
            },
        },
    );
    assert!(document.template().is_err());
}
#[test]
fn supported_slab_state_and_pot_contents_survive_reload() {
    let mut document = Document::default();
    document.cells.insert(
        [1, 0, 1],
        AuthoredCell {
            material: 105,
            state: BlockObjectState {
                facing: 1,
                ..Default::default()
            },
        },
    );
    document.cells.insert(
        [2, 0, 1],
        AuthoredCell {
            material: terra_core::flower_pot::MATERIAL,
            state: BlockObjectState {
                facing: 1,
                ..Default::default()
            },
        },
    );
    let template = document.template().unwrap();
    let loaded = Document::from_template(template, None).unwrap();
    assert_eq!(loaded.cells, document.cells);
}
#[test]
fn json_files_are_create_only_until_overwrite_is_confirmed() {
    let directory = std::env::temp_dir().join(format!(
        "terra-editor-files-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("test-building.json");
    let mut document = Document::default();
    document.cells.insert([0, 0, 0], cell(43));
    let mut template = document.template().unwrap();
    save_template(&path, &template, false).unwrap();
    let before = std::fs::read(&path).unwrap();
    template.display_name = "Changed".into();
    assert!(save_template(&path, &template, false).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    save_template(&path, &template, true).unwrap();
    assert_eq!(BuildingTemplate::load(&path).unwrap(), template);
    assert!(save_template(&directory.join("INVALID!.json"), &template, false).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn all_saved_stair_poses_and_mixed_slab_inputs_use_shared_schema_semantics() {
    for facing in 0..24 {
        let mut document = Document::default();
        document.cells.insert(
            [0, 0, 0],
            AuthoredCell {
                material: 74,
                state: BlockObjectState {
                    facing,
                    ..Default::default()
                },
            },
        );
        let template = document.template().unwrap();
        let cell = &template.steps[0].cells[0];
        assert!(cell.rotate_facing);
        assert_eq!(cell.facing, facing);
        let placed = template.placed([0; 3], 1).unwrap();
        assert_eq!(
            placed[0].cells[0].facing,
            terra_core::building_jobs::template::rotated_facing(cell, 1).unwrap()
        );
    }
    let facing = terra_core::construction::merged_slab(82, 5, 83).unwrap();
    let mut document = Document::default();
    document.cells.insert(
        [0, 0, 0],
        AuthoredCell {
            material: 82,
            state: BlockObjectState {
                facing,
                ..Default::default()
            },
        },
    );
    let template = document.template().unwrap();
    assert_eq!(
        template
            .materials
            .iter()
            .map(|s| (s.item, s.count))
            .collect::<Vec<_>>(),
        [(ItemKind::Block(82), 1), (ItemKind::Block(83), 1)]
    );
    assert_eq!(
        Document::from_template(template, None).unwrap().cells,
        document.cells
    );
}
#[test]
fn paired_chests_keep_both_cells_and_cost_two_actual_chests() {
    let mut document = Document::default();
    document.cells.insert(
        [0, 0, 0],
        AuthoredCell {
            material: 29,
            state: BlockObjectState {
                peer: Some([1, 0, 0]),
                ..Default::default()
            },
        },
    );
    document.cells.insert(
        [1, 0, 0],
        AuthoredCell {
            material: 29,
            state: BlockObjectState {
                peer: Some([0, 0, 0]),
                secondary: true,
                ..Default::default()
            },
        },
    );
    let template = document.template().unwrap();
    assert_eq!(
        template.steps[0].costs,
        [terra_core::ItemStack {
            item: ItemKind::Block(29),
            count: 2
        }]
    );
    assert_eq!(
        Document::from_template(template, None).unwrap().cells,
        document.cells
    );
}

#[test]
fn saving_into_a_game_catalog_rejects_duplicate_ids_but_allows_replacing_the_original() {
    let root = std::env::temp_dir().join(format!("terra-editor-catalog-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let original = root.join("original.json");
    let target = root.join("copy.json");
    let mut document = Document::default();
    document.cells.insert([0, 0, 0], cell(43));
    let template = document.template().unwrap();
    save_template(&original, &template, true).unwrap();
    assert!(files::ensure_catalog_identity(&root, &target, &template.id).is_err());
    assert!(files::ensure_catalog_identity(&root, &original, &template.id).is_ok());
    assert!(files::ensure_catalog_identity(&root, &target, "distinct-house").is_ok());
    std::fs::remove_dir_all(root).unwrap();
}
