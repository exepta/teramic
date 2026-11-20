use super::*;
use terra_core::{
    ItemKind,
    building_jobs::{HOUSE_TEMPLATE, template::TemplateCategory},
};

fn summary() -> TemplateSummary {
    TemplateSummary {
        id: HOUSE_TEMPLATE.into(),
        display_name: "Village house".into(),
        category: TemplateCategory::Residential,
        category_path: String::new(),
        dimensions: [9, 11, 11],
        steps: 390,
        materials: vec![ItemStack {
            item: ItemKind::Block(16),
            count: 200,
        }],
    }
}

#[test]
fn template_selection_and_explicit_aggregate_material_preview_roundtrip_without_inventory_stack_limits()
 {
    let action = BuildingAction::QueueHouse {
        kingdom: KingdomId(1),
        origin: [-1, 12, -32],
        facing: 3,
        template: "spruce-cottage-v1".into(),
    };
    let decoded: BuildingAction =
        serde_json::from_str(&serde_json::to_string(&action).unwrap()).unwrap();
    assert_eq!(decoded, action);
    let snapshot = BuildingSnapshot {
        templates: vec![summary()],
        ..Default::default()
    };
    assert!(snapshot.valid());
    let decoded: BuildingSnapshot =
        serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(
        decoded.template(HOUSE_TEMPLATE).unwrap().materials[0].count,
        200
    );
    for bad in ["../residential.json", "/tmp/house", "", "Unknown"] {
        let mut invalid = snapshot.clone();
        invalid.templates[0].id = bad.into();
        assert!(!invalid.valid());
    }
    let mut duplicate = snapshot.clone();
    duplicate.templates.push(summary());
    assert!(!duplicate.valid());
    let mut too_many = snapshot.clone();
    too_many.templates = (0..=MAX_TEMPLATES)
        .map(|i| TemplateSummary {
            id: format!("house-{i}"),
            ..summary()
        })
        .collect();
    assert!(!too_many.valid());
    let mut error = BuildingSnapshot {
        catalog_error: Some("x".repeat(512)),
        ..Default::default()
    };
    assert!(error.valid());
    error.catalog_error.as_mut().unwrap().push('x');
    assert!(!error.valid());
}

#[test]
fn resolved_legacy_and_retired_jobs_remain_valid_without_their_template_in_the_live_catalog() {
    let owner = PlayerUuid::from_u128(1);
    for id in [HOUSE_TEMPLATE, "retired-residential-v1"] {
        let snapshot = BuildingSnapshot {
            settlements: BTreeMap::from([(KingdomId(1), SettlementBuilding::new(owner))]),
            jobs: BTreeMap::from([(
                1,
                BuildingJobView {
                    id: 1,
                    kingdom: KingdomId(1),
                    owner,
                    origin: [-1, 12, -32],
                    facing: 0,
                    template: id.into(),
                    workers: vec![],
                    progress: 0.2,
                    required_materials: vec![ItemStack {
                        item: ItemKind::Block(16),
                        count: 300,
                    }],
                    status: BuildStatus::Paused,
                    enabled: false,
                    revision: 1,
                    work_seconds: 360.0,
                    free_resources: false,
                    remaining_seconds: 1440.0,
                },
            )]),
            ..Default::default()
        };
        assert!(snapshot.valid());
        assert_eq!(
            snapshot.settlements[&KingdomId(1)].selected_template,
            HOUSE_TEMPLATE
        );
        assert!(snapshot.template(id).is_none());
    }
}

#[test]
fn the_largest_allowed_catalog_has_a_bounded_metadata_payload_without_transmitting_voxels() {
    let materials: Vec<_> = (1..=u16::MAX)
        .map(ItemKind::Block)
        .filter(|item| item.valid())
        .take(64)
        .map(|item| ItemStack { item, count: 200 })
        .collect();
    assert_eq!(materials.len(), 64);
    let snapshot = BuildingSnapshot {
        templates: (0..MAX_TEMPLATES)
            .map(|index| TemplateSummary {
                id: format!("residential-{index}"),
                display_name: "A".repeat(80),
                category: TemplateCategory::Residential,
                category_path: String::new(),
                dimensions: [9, 12, 11],
                steps: 8192,
                materials: materials.clone(),
            })
            .collect(),
        ..Default::default()
    };
    assert!(snapshot.valid());
    assert!(
        serde_json::to_vec(&snapshot).unwrap().len() < 128 * 1024,
        "even the verbose JSON catalog remains below128KiB; voxel steps are never published"
    );
}

#[test]
fn resource_and_preview_snapshots_are_explicitly_tagged_to_the_authorized_kingdom() {
    let id = KingdomId(1);
    let owner = PlayerUuid::from_u128(240);
    let source = terra_core::building_jobs::template::BuildingTemplate::load(
        &terra_core::asset_directory()
            .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH),
    )
    .unwrap();
    let mut stock = TownStock {
        configured: true,
        complete: true,
        chests: 1,
        items: BTreeMap::from([(ItemKind::Block(16), 200)]),
        ..Default::default()
    };
    stock.summarize();
    let snapshot = BuildingSnapshot {
        stock_kingdom: Some(id),
        stock,
        preview: Some(TemplatePreview::from_template(&source).unwrap()),
        unlimited_resources: true,
        settlements: BTreeMap::from([(id, SettlementBuilding::new(owner))]),
        ..Default::default()
    };
    assert!(snapshot.valid());
    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: BuildingSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, snapshot);
    let binary = postcard::to_allocvec(&snapshot).unwrap();
    assert_eq!(
        postcard::from_bytes::<BuildingSnapshot>(&binary).unwrap(),
        snapshot
    );
    let mut duplicate = serde_json::to_value(&snapshot.stock).unwrap();
    let entries = duplicate.get_mut("items").unwrap().as_array_mut().unwrap();
    entries.push(entries[0].clone());
    assert!(serde_json::from_value::<TownStock>(duplicate).is_err());
    let mut forged = snapshot.clone();
    forged.stock_kingdom = Some(KingdomId(999));
    assert!(!forged.valid());
    forged = snapshot;
    forged.stock_kingdom = None;
    assert!(!forged.valid());
    let action = BuildingAction::Preview {
        kingdom: id,
        template: HOUSE_TEMPLATE.into(),
    };
    assert_eq!(
        serde_json::from_str::<BuildingAction>(&serde_json::to_string(&action).unwrap()).unwrap(),
        action
    );
}
