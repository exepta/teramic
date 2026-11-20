use super::*;

fn simple() -> BuildingTemplate {
    let steps = vec![TemplateStep {
        phase: BuildPhase::Floor,
        cells: vec![TemplateCell {
            position: [0, -1, 0],
            material: 3,
            facing: 0,
            rotate_facing: false,
            peer: None,
            secondary: false,
        }],
        costs: vec![ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        }],
    }];
    BuildingTemplate {
        schema_version: TEMPLATE_SCHEMA,
        id: HOUSE_TEMPLATE.into(),
        display_name: "Temperate village house".into(),
        category: TemplateCategory::Residential,
        anchor: [0; 3],
        bounds: BuildArea::new([0, -1, 0], [0; 3]).unwrap(),
        materials: material_totals(&steps).unwrap(),
        steps,
    }
}

#[test]
fn template_json_roundtrip_keeps_every_relative_cell_input_and_anchor() {
    let template = simple();
    let text = serde_json::to_string(&template).unwrap();
    assert_eq!(BuildingTemplate::from_json(&text).unwrap(), template);
    assert_eq!(
        template.placed([-1, 12, -32], 3).unwrap()[0].cells[0].position,
        [-1, 11, -32]
    );
    let mut shifted = template.clone();
    shifted.anchor = [100, 20, -7];
    shifted.bounds = BuildArea::new([100, 19, -7], [100, 20, -7]).unwrap();
    shifted.steps[0].cells[0].position = [100, 19, -7];
    assert_eq!(
        shifted.placed([-1, 12, -32], 3).unwrap(),
        template.placed([-1, 12, -32], 3).unwrap()
    );
}

#[test]
fn malformed_versions_unknown_fields_and_unbounded_or_duplicate_cells_are_rejected() {
    let mut template = simple();
    template.schema_version = 2;
    assert!(template.validate().is_err());
    template = simple();
    template.steps.push(template.steps[0].clone());
    template.materials = material_totals(&template.steps).unwrap();
    assert!(template.validate().unwrap_err().contains("duplicate"));
    template = simple();
    template.steps[0].cells[0].position = [100, 0, 0];
    assert!(template.validate().is_err());
    let text = serde_json::to_string(&simple()).unwrap();
    assert!(BuildingTemplate::from_json(&text.replacen('{', "{\"ignored\":true,", 1)).is_err());
    assert!(BuildingTemplate::from_json(&" ".repeat(MAX_TEMPLATE_BYTES + 1)).is_err());
}

#[test]
fn actual_material_inputs_are_required_and_aggregate_counts_cannot_be_fabricated() {
    let mut template = simple();
    template.steps[0].costs.clear();
    assert!(template.validate().unwrap_err().contains("inputs"));
    template = simple();
    template.materials[0].count = 2;
    assert!(template.validate().unwrap_err().contains("material list"));
    template = simple();
    template.steps[0].cells[0].material = 0;
    assert!(template.validate().is_err());
}

#[test]
fn templates_and_frozen_worker_steps_cannot_stamp_tar_onto_unprepared_cells() {
    // A correctly priced custom template must still use the shovel/bucket
    // workflow. Creative workers otherwise bypass its unavailable item cost.
    for (material, facing, allowed) in [
        (102, 0, true),
        (crate::roads::ASPHALT, 0, true),
        (crate::roads::WET_TAR, 0, false),
        (crate::roads::WET_TAR, 255, false),
        (crate::roads::DRIED_TAR, 0, false),
        (crate::roads::DRIED_TAR, 7, false),
    ] {
        let mut template = simple();
        let cell = &mut template.steps[0].cells[0];
        cell.material = material;
        cell.facing = facing;
        template.steps[0].costs[0].item = ItemKind::Block(material);
        template.materials = template.steps[0].costs.clone();
        assert_eq!(template.validate().is_ok(), allowed);
        assert_eq!(cell_inputs(&template.steps[0].cells).is_ok(), allowed);
        assert_eq!(template.placed([0; 3], 0).is_ok(), allowed);
        assert_eq!(
            BuildingTemplate::from_json(&serde_json::to_string(&template).unwrap()).is_ok(),
            allowed,
        );
        let frozen = BuildStep {
            phase: BuildPhase::Floor,
            cells: vec![BuildCell {
                position: [0, -1, 0],
                material,
                facing,
                peer: None,
                secondary: false,
            }],
            costs: template.steps[0].costs.clone(),
        };
        assert_eq!(frozen.valid(), allowed);
    }
}

#[test]
fn reciprocal_paired_halves_rotate_with_one_real_item_cost() {
    let mut template = simple();
    template.steps = vec![TemplateStep {
        phase: BuildPhase::Furnishing,
        cells: vec![
            TemplateCell {
                position: [0, 0, 0],
                material: 65,
                facing: 0,
                rotate_facing: true,
                peer: Some([0, 1, 0]),
                secondary: false,
            },
            TemplateCell {
                position: [0, 1, 0],
                material: 65,
                facing: 0,
                rotate_facing: true,
                peer: Some([0, 0, 0]),
                secondary: true,
            },
        ],
        costs: vec![ItemStack {
            item: ItemKind::Block(65),
            count: 1,
        }],
    }];
    template.bounds = BuildArea::new([0, 0, 0], [0, 1, 0]).unwrap();
    template.materials = material_totals(&template.steps).unwrap();
    let placed = template.placed([-16, 12, -1], 3).unwrap();
    assert_eq!(placed[0].cells[0].facing, 3);
    assert_eq!(placed[0].cells[0].peer, Some([-16, 13, -1]));
    assert_eq!(placed[0].costs[0].count, 1);
    template.steps[0].cells[1].peer = None;
    assert!(template.validate().unwrap_err().contains("paired"));
}

#[test]
fn shipped_house_is_real_json_with_materials_and_object_pairs() {
    let template =
        BuildingTemplate::load(&crate::asset_directory().join(DEFAULT_HOUSE_PATH)).unwrap();
    assert!(template.steps.len() > 200);
    assert!(template.steps.iter().any(|step| step.cells.len() == 2));
    assert!(
        template
            .steps
            .iter()
            .any(|step| step.cells[0].material == crate::flower_pot::MATERIAL
                && step.costs.len() == 2)
    );
    assert!(
        template
            .materials
            .iter()
            .any(|stack| stack.item == ItemKind::Block(43))
    );
    assert!(!template.materials.is_empty());
}

#[test]
fn loading_an_edited_file_uses_its_validated_contents_and_never_reconstructs_defaults() {
    let path =
        std::env::temp_dir().join(format!("terra-template-test-{}.json", std::process::id()));
    let mut template = simple();
    template.steps[0].cells[0].material = 16;
    template.steps[0].costs[0].item = ItemKind::Block(16);
    template.materials = material_totals(&template.steps).unwrap();
    std::fs::write(&path, serde_json::to_string(&template).unwrap()).unwrap();
    let loaded = BuildingTemplate::load(&path).unwrap();
    assert_eq!(loaded.placed([0; 3], 0).unwrap()[0].cells[0].material, 16);
    std::fs::write(&path, "{broken}").unwrap();
    assert!(
        BuildingTemplate::load(&path)
            .unwrap_err()
            .contains(&path.display().to_string())
    );
    std::fs::remove_file(&path).unwrap();
    assert!(BuildingTemplate::load(&path).is_err());
}

#[test]
fn buried_wall_and_roof_steps_are_rejected_instead_of_disappearing_from_resolved_plans() {
    for phase in [BuildPhase::Walls, BuildPhase::Roof] {
        let mut template = simple();
        template.steps[0].phase = phase;
        assert!(
            template
                .validate()
                .unwrap_err()
                .contains("nonnegative local height")
        );
        // Validation is relative to the anchor, including shifted authoring coordinates.
        template.anchor = [100, 20, -7];
        template.bounds = BuildArea::new([100, 19, -7], [100, 20, -7]).unwrap();
        template.steps[0].cells[0].position = [100, 19, -7];
        assert!(template.validate().is_err());
        template.steps[0].cells[0].position[1] = 20;
        assert!(template.validate().is_ok());
    }
}

#[test]
fn permanent_cells_cannot_collide_with_the_temporary_interior_stair_or_working_deck() {
    for position in [[-2, 0, -2], [-2, 1, -1], [0, 3, 0]] {
        let mut template = simple();
        template.bounds = BuildArea::new([-2, -1, -2], [0, 3, 0]).unwrap();
        template.steps[0].phase = BuildPhase::Walls;
        template.steps[0].cells[0].position = position;
        assert!(
            template
                .validate()
                .unwrap_err()
                .contains("reserved interior working space")
        );
        template.steps[0].phase = BuildPhase::Furnishing;
        assert!(
            template.validate().is_ok(),
            "furniture is installed after scaffold removal"
        );
    }
}
